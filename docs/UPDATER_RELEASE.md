# Guia de releases y updater

Esta guia explica como preparar una release de DB Viewer para que el updater de
Tauri pueda descargarla desde GitHub Releases.

## Resumen rapido

El updater funciona asi:

1. La app instalada ejecuta `check()` desde el boton de actualizaciones.
2. Tauri descarga `latest.json` desde GitHub:

   ```text
   https://github.com/IvanG11/db-viewer/releases/latest/download/latest.json
   ```

3. Si `latest.json` contiene una version mayor que la version instalada, Tauri
   descarga el bundle indicado en el JSON.
4. Tauri valida la firma con el `pubkey` de `src-tauri/tauri.conf.json`.
5. Si la firma es correcta, instala la actualizacion y relanza la app.

## Punto importante para la primera prueba

Si `0.6.2` es la primera version que incluye updater, entonces esa version debe
estar instalada localmente antes de poder probar una actualizacion.

El flujo correcto para probar es:

1. Compilar e instalar `0.6.2` con el updater ya integrado.
2. Subir una release `v0.6.2` a GitHub, si quieres dejarla publicada como base.
3. Subir despues una release nueva, por ejemplo `v0.6.3`.
4. Abrir la app instalada `0.6.2`.
5. Pulsar el boton de actualizar.
6. La app `0.6.2` deberia detectar `0.6.3`, descargarla e instalarla.

Una app anterior que no tenga este updater integrado no puede actualizarse por
este sistema, porque no tiene codigo que consulte GitHub ni que valide firmas.

## Clave privada de firma

El archivo `src-tauri/tauri.conf.json` contiene el `pubkey`. Por eso, al hacer
un build de release, Tauri exige una clave privada para firmar los artifacts.

Para desarrollo local, se puede usar una clave guardada fuera del repo:

```bash
mkdir -p "$HOME/.config/db-viewer"
cp /tmp/db-viewer-updater.key "$HOME/.config/db-viewer/updater.key"
```

Luego compila asi:

```bash
npm run build:app:signed
```

Ese comando usa por defecto:

```text
~/.config/db-viewer/updater.key
```

Si la clave esta en otro sitio, puedes seguir usando
`TAURI_SIGNING_PRIVATE_KEY_PATH` para indicarle al script otra ruta. El script
lee el archivo y pasa el contenido a Tauri con `TAURI_SIGNING_PRIVATE_KEY`.

No subas nunca la clave privada al repo ni a un release publico. En GitHub
Actions debe guardarse como secret.

## Preparar una nueva version

Ejemplo: publicar `0.6.4`.

Actualiza la version con:

```bash
npm run version:set -- 0.6.4
```

Ese comando cambia la version en `package.json`, `package-lock.json`,
`src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`, y despues ejecuta
`cargo check` para refrescar `src-tauri/Cargo.lock`.

Comprueba que no queda una version antigua:

```bash
rg "0.6.3|0.6.4" package.json package-lock.json src-tauri/Cargo.toml src-tauri/tauri.conf.json
```

## Compilar la app firmada

Ejecuta:

```bash
npm run build:app:signed
```

En Linux, si el bundle AppImage funciona, Tauri deberia generar archivos dentro
de:

```text
src-tauri/target/release/bundle/appimage/
```

Los archivos importantes seran parecidos a:

```text
db-viewer_0.6.4_amd64.AppImage
db-viewer_0.6.4_amd64.AppImage.sig
```

Si aparece este error:

```text
A public key has been found, but no private key.
```

significa que falta `TAURI_SIGNING_PRIVATE_KEY_PATH` o
`TAURI_SIGNING_PRIVATE_KEY`.

Si aparece este error:

```text
failed to run linuxdeploy
```

la app Rust ya ha compilado, pero fallo el empaquetado AppImage. En ese caso hay
que revisar la instalacion local de dependencias de Tauri/linuxdeploy antes de
poder crear los assets finales.

## Crear latest.json

El archivo `latest.json` es el indice que lee el updater. Normalmente no hace
falta crearlo a mano: despues de `npm run build:app`, ejecuta el comando de
publicacion:

```bash
npm run release:github
```

Ese comando:

1. Busca el AppImage de la version actual.
2. Busca su `.sig`.
3. Genera `latest.json` con la firma dentro.
4. Crea la release en GitHub si no existe.
5. Si la release ya existe, sube/reemplaza los assets.

Para probar sin subir nada:

```bash
npm run release:github -- --dry-run
```

Tambien puedes indicar datos manualmente:

```bash
npm run release:github -- --version 0.6.4 --tag v0.6.4 --notes "Test updater"
```

Si necesitas publicar en otro repo:

```bash
npm run release:github -- --repo IvanG11/db-viewer
```

El comando usa GitHub CLI, asi que antes debes iniciar sesion:

```bash
gh auth login
```

Si prefieres hacerlo a mano, usa este formato.

Para Linux x86_64, usa este formato:

```json
{
  "version": "0.6.4",
  "notes": "Test updater",
  "pub_date": "2026-05-02T00:00:00Z",
  "platforms": {
    "linux-x86_64": {
      "signature": "CONTENIDO_DEL_ARCHIVO_SIG",
      "url": "https://github.com/IvanG11/db-viewer/releases/download/v0.6.4/db-viewer_0.6.4_amd64.AppImage"
    }
  }
}
```

La propiedad `signature` no es una ruta. Debe ser el contenido del archivo
`.sig`.

Para obtenerlo:

```bash
cat src-tauri/target/release/bundle/appimage/db-viewer_0.6.4_amd64.AppImage.sig
```

Copia ese contenido en `latest.json`.

El script automatico guarda `latest.json` en:

```text
src-tauri/target/release/bundle/appimage/latest.json
```

## Subir la release desde la web de GitHub

1. Entra en:

   ```text
   https://github.com/IvanG11/db-viewer
   ```

2. Abre la seccion **Releases**.
3. Pulsa **Draft a new release**.
4. Crea el tag:

   ```text
   v0.6.4
   ```

5. Usa como titulo:

   ```text
   v0.6.4
   ```

6. En assets, sube:

   ```text
   db-viewer_0.6.4_amd64.AppImage
   db-viewer_0.6.4_amd64.AppImage.sig
   latest.json
   ```

7. Pulsa **Publish release**.

Cuando esta release sea la ultima publicada, GitHub servira el JSON en:

```text
https://github.com/IvanG11/db-viewer/releases/latest/download/latest.json
```

## Subir la release con GitHub CLI

Si tienes `gh` instalado:

```bash
gh auth login
```

Luego:

```bash
gh release create v0.6.4 \
  src-tauri/target/release/bundle/appimage/db-viewer_0.6.4_amd64.AppImage \
  src-tauri/target/release/bundle/appimage/db-viewer_0.6.4_amd64.AppImage.sig \
  latest.json \
  --title "v0.6.4" \
  --notes "Test updater"
```

Para reemplazar assets en una release existente:

```bash
gh release upload v0.6.4 latest.json --clobber
```

## Publicar con GitHub Actions

El repo incluye un workflow en:

```text
.github/workflows/release.yml
```

Cuando subes un tag `v*`, GitHub Actions:

1. Instala dependencias.
2. Comprueba que el tag coincide con la version de `package.json`.
3. Compila la AppImage firmada.
4. Genera `latest.json`.
5. Publica o actualiza la GitHub Release.

### Secrets necesarios

En GitHub, ve a **Settings > Secrets and variables > Actions** y crea:

```text
TAURI_SIGNING_PRIVATE_KEY
```

Debe contener el contenido completo de la clave privada:

```bash
cat "$HOME/.config/db-viewer/updater.key"
```

Si tu clave tiene password, crea tambien:

```text
TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

Si la clave no tiene password, no hace falta crear ese secret.

### Flujo recomendado

Para publicar una version nueva:

```bash
npm run version:set -- 0.6.8
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "Release 0.6.8"
git tag v0.6.8
git push origin main
git push origin v0.6.8
```

Despues mira el progreso en **Actions** dentro de GitHub.

Evita `git push origin main --tags` para releases. Si tienes varios tags locales
sin subir, GitHub puede recibir muchos tags de golpe y no disparar el workflow de
release para ellos.

## Test manual completo

Para probar `0.6.2 -> 0.6.3`:

1. Deja el codigo en version `0.6.2`.
2. Compila `0.6.2` con clave privada.
3. Instala o ejecuta el AppImage `0.6.2`.
4. Cambia el codigo a version `0.6.3`.
5. Compila `0.6.3` con la misma clave privada.
6. Crea `latest.json` apuntando al AppImage `0.6.3`.
7. Publica la release `v0.6.3` en GitHub con el AppImage, el `.sig` y
   `latest.json`.
8. Abre la app instalada `0.6.2`.
9. Pulsa el boton de actualizar en la barra superior.
10. Confirma la descarga.
11. La app deberia instalar la actualizacion y relanzarse como `0.6.3`.

## Checklist para cada release

- La version esta actualizada en `package.json`, `package-lock.json`,
  `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`.
- El build se hizo con la misma clave privada que corresponde al `pubkey`.
- El `.sig` se genero junto al bundle.
- `latest.json` tiene una version mayor que la instalada.
- `latest.json` contiene el contenido del `.sig`, no una ruta al archivo.
- La URL del bundle en `latest.json` apunta al asset real de GitHub Release.
- La release esta publicada, no solo guardada como draft.
