# Export Bundle y Ejecutable Windows v1

Owner: Release Engineering

## Problema

1. Sin pipeline unico, release depende de pasos manuales.
2. Falta contrato formal de bundle ejecutable funcional.

## Contrato Package Windows v1

1. Comando canonico:
2. `vnengine package --project <root> --target windows-x64 --out <dir>`

### Layout minimo

1. `runtime/` artefacto runtime.
2. `scripts/` script canonico y compilado.
3. `assets/` recursos validados.
4. `meta/project.vnm`.
5. `meta/assets_manifest.json`.
6. `meta/package_report.json`.
7. launcher (`launch.bat` o equivalente).

### Integridad

1. hash manifest obligatorio.
2. firma opcional v1, obligatoria v2 release.

## Reglas normativas

| Rule ID | Requirement | Owner | Metric | Gate | Evidence |
|---|---|---|---|---|---|
| PKG-001 | `package` genera layout completo | Release Eng | 100% archivos requeridos | package command tests | package report |
| PKG-002 | Smoke-run artefacto windows exitoso | QA Lead | 100% smoke tests pass | CI windows gate | smoke logs |
| PKG-003 | Manifest de assets hash valido | Security Lead | 100% entradas hash verificables | integrity tests | manifest verify report |
| PKG-004 | Reporte estructurado versionado | Tooling Lead | 100% bundles con schema | schema validator | package_report.json |
| PKG-005 | Reproducibilidad de artefacto | Release Eng | hash reproducible dentro de tolerancia | reproducibility gate | build hash report |

## No objetivos v1

1. soporte multiplataforma completo.
2. firma obligatoria en todos los perfiles.

## Estado actual observado

1. `story_first` mejora import significativamente.
2. `full` aun genera ruido alto y requiere fase de reduccion adicional.
