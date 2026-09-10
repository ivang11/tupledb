use super::sql;
use crate::database::{
    sql::SqlDialect,
    types::{DatabaseCollation, DatabaseCreationOptions},
};
use serde_json::Value;
use sqlx::{PgPool, Row};

struct Locale {
    provider: String,
    collate: String,
    ctype: String,
    locale: Option<String>,
    rules: Option<String>,
}

struct Collation {
    name: String,
    encodings: Vec<String>,
    locale: Locale,
}

pub(super) struct Options {
    version: i32,
    default_encoding: String,
    default_locale: Locale,
    collations: Vec<Collation>,
}

impl Options {
    pub async fn load(pool: &PgPool) -> Result<Self, String> {
        // template1, not the initial connection's database, defines CREATE defaults.
        // JSON field access bridges the ICU catalog field rename in PostgreSQL 17.
        let defaults = sqlx::query("SELECT pg_encoding_to_char(encoding) AS encoding,
            datcollate, datctype, COALESCE(to_jsonb(d)->>'datlocprovider','c') AS provider,
            COALESCE(to_jsonb(d)->>'datlocale',to_jsonb(d)->>'daticulocale') AS locale,
            to_jsonb(d)->>'daticurules' AS rules, current_setting('server_version_num')::int AS version
            FROM pg_catalog.pg_database d WHERE datname='template1'")
            .fetch_one(pool).await.map_err(|e| e.to_string())?;
        // PostgreSQL's server encoding IDs end at KOI8U; subsequent IDs are
        // client-only encodings (e.g. SJIS), not valid database encodings.
        let encodings: Vec<String> = sqlx::query_scalar(
            "SELECT pg_encoding_to_char(id)
            FROM generate_series(0,pg_char_to_encoding('KOI8U')) AS e(id) ORDER BY 1",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        let version: i32 = defaults.get("version");
        let rows = sqlx::query(
            "SELECT n.nspname::text AS schema, c.collname::text AS name,
            c.collprovider::text AS provider, c.collencoding AS encoding,
            pg_encoding_to_char(c.collencoding) AS encoding_name,
            COALESCE(c.collcollate,'') AS collate, COALESCE(c.collctype,'') AS ctype,
            COALESCE(to_jsonb(c)->>'colllocale',to_jsonb(c)->>'colliculocale') AS locale,
            to_jsonb(c)->>'collicurules' AS rules
            FROM pg_catalog.pg_collation c JOIN pg_catalog.pg_namespace n ON n.oid=c.collnamespace
            WHERE c.collisdeterministic AND c.collprovider IN ('c','i','b')
            AND has_schema_privilege(n.oid,'USAGE') ORDER BY n.nspname,c.collname,c.collencoding",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
        let mut collations = Vec::new();
        for row in rows {
            let provider: String = row.get("provider");
            if (provider == "i" && version < 150000) || (provider == "b" && version < 170000) {
                continue;
            }
            let encoding: i32 = row.get("encoding");
            let available = if provider != "c" {
                // Offer portable UTF8 database configurations for ICU/builtin.
                vec!["UTF8".to_owned()]
            } else if encoding == -1 {
                encodings.clone()
            } else {
                vec![row.get("encoding_name")]
            };
            collations.push(Collation {
                name: format!(
                    "{}.{}",
                    sql::quote_identifier(row.get("schema"))?,
                    sql::quote_identifier(row.get("name"))?
                ),
                encodings: available,
                locale: Locale {
                    provider,
                    collate: row.get("collate"),
                    ctype: row.get("ctype"),
                    locale: row.get("locale"),
                    rules: row.get("rules"),
                },
            });
        }
        Ok(Self {
            version,
            default_encoding: defaults.get("encoding"),
            default_locale: Locale {
                provider: defaults.get("provider"),
                collate: defaults.get("datcollate"),
                ctype: defaults.get("datctype"),
                locale: defaults.get("locale"),
                rules: defaults.get("rules"),
            },
            collations,
        })
    }

    fn is_default(&self, locale: &Locale) -> bool {
        let default = &self.default_locale;
        locale.provider == default.provider
            && if locale.provider == "c" {
                locale.collate == default.collate && locale.ctype == default.ctype
            } else {
                locale.locale == default.locale && locale.rules == default.rules
            }
    }

    pub fn public(&self) -> DatabaseCreationOptions {
        DatabaseCreationOptions {
            default_character_set: self.default_encoding.clone(),
            default_collation: self
                .collations
                .iter()
                .find(|c| self.is_default(&c.locale))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| {
                    self.default_locale
                        .locale
                        .clone()
                        .unwrap_or_else(|| self.default_locale.collate.clone())
                }),
            collations: self
                .collations
                .iter()
                .flat_map(|c| {
                    c.encodings.iter().map(|encoding| DatabaseCollation {
                        name: c.name.clone(),
                        character_set: encoding.clone(),
                        is_default: self.is_default(&c.locale),
                    })
                })
                .collect(),
        }
    }

    pub fn create_sql(
        &self,
        name: &str,
        encoding: Option<&str>,
        collation: Option<&str>,
    ) -> Result<String, String> {
        let mut query = format!("CREATE DATABASE {}", sql::quote_identifier(name)?);
        if encoding.is_none() && collation.is_none() {
            return Ok(query);
        }
        let encoding = encoding.unwrap_or(&self.default_encoding);
        if !self
            .collations
            .iter()
            .any(|c| c.encodings.iter().any(|e| e == encoding))
        {
            return Err(format!("Unsupported PostgreSQL encoding: {encoding}"));
        }
        let locale = match collation {
            Some(name) => {
                &self
                    .collations
                    .iter()
                    .find(|c| c.name == name && c.encodings.iter().any(|e| e == encoding))
                    .ok_or_else(|| {
                        format!("Collation {name} is not available for encoding {encoding}")
                    })?
                    .locale
            }
            // The server's default locale (e.g. libc en_US.UTF-8) is only valid
            // for its own encoding; find the matching default collation entry
            // that is actually available for the requested encoding instead of
            // pairing an arbitrary locale with an incompatible ENCODING clause.
            None => {
                self.collations
                    .iter()
                    .find(|c| self.is_default(&c.locale) && c.encodings.iter().any(|e| e == encoding))
                    .map(|c| &c.locale)
                    .ok_or_else(|| {
                        format!(
                            "The server's default collation is not available for encoding {encoding}; choose a collation explicitly"
                        )
                    })?
            }
        };
        let literal = |value: &str| sql::PostgreSqlDialect.literal(&Value::String(value.into()));
        query.push_str(&format!(
            " TEMPLATE template0 ENCODING {}",
            literal(encoding)
        ));
        match locale.provider.as_str() {
            "c" => {
                if self.version >= 150000 {
                    query.push_str(" LOCALE_PROVIDER libc");
                }
                query.push_str(&format!(
                    " LC_COLLATE {} LC_CTYPE {}",
                    literal(&locale.collate),
                    literal(&locale.ctype)
                ));
            }
            "i" | "b" => {
                let identifier = locale
                    .locale
                    .as_deref()
                    .ok_or("Missing PostgreSQL locale metadata")?;
                if locale.provider == "i" {
                    query.push_str(&format!(
                        " LOCALE_PROVIDER icu ICU_LOCALE {}",
                        literal(identifier)
                    ));
                    if let Some(rules) = &locale.rules {
                        if !rules.is_empty() {
                            query.push_str(&format!(" ICU_RULES {}", literal(rules)));
                        }
                    }
                } else {
                    query.push_str(&format!(
                        " LOCALE_PROVIDER builtin BUILTIN_LOCALE {}",
                        literal(identifier)
                    ));
                }
                // ICU/builtin do not derive their ordering from libc; C is
                // available independently of the operating system's locale list.
                query.push_str(" LC_COLLATE 'C' LC_CTYPE 'C'");
            }
            _ => return Err("Unsupported PostgreSQL locale provider".into()),
        }
        Ok(query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale(provider: &str, collate: &str, ctype: &str) -> Locale {
        Locale {
            provider: provider.into(),
            collate: collate.into(),
            ctype: ctype.into(),
            locale: None,
            rules: None,
        }
    }

    fn options() -> Options {
        Options {
            version: 160000,
            default_encoding: "UTF8".into(),
            default_locale: locale("c", "en_US.UTF-8", "en_US.UTF-8"),
            collations: vec![
                Collation {
                    name: "\"pg_catalog\".\"en_US\"".into(),
                    encodings: vec!["UTF8".into()],
                    locale: locale("c", "en_US.UTF-8", "en_US.UTF-8"),
                },
                Collation {
                    name: "\"pg_catalog\".\"C\"".into(),
                    encodings: vec!["UTF8".into(), "LATIN1".into()],
                    locale: locale("c", "C", "C"),
                },
            ],
        }
    }

    #[test]
    fn create_sql_rejects_an_encoding_the_default_collation_cannot_support() {
        let options = options();
        let err = options.create_sql("db", Some("LATIN1"), None).unwrap_err();
        assert!(err.contains("default collation is not available for encoding LATIN1"));
    }

    #[test]
    fn create_sql_uses_an_explicit_collation_compatible_with_the_encoding() {
        let options = options();
        let sql = options
            .create_sql("db", Some("LATIN1"), Some("\"pg_catalog\".\"C\""))
            .unwrap();
        assert!(sql.contains("LC_COLLATE E'C' LC_CTYPE E'C'"));
    }

    #[test]
    fn create_sql_resolves_the_default_collation_for_the_default_encoding() {
        let options = options();
        let sql = options.create_sql("db", Some("UTF8"), None).unwrap();
        assert!(sql.contains("LC_COLLATE E'en_US.UTF-8' LC_CTYPE E'en_US.UTF-8'"));
    }
}
