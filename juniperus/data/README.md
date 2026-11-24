# Datos de Juniperus

Los datasets generados **no se commitean**. Este directorio sólo guarda la
estructura y el contrato.

```
data/
├── raw/          # respuestas crudas del profesor (opcional, ignorado)
└── processed/    # train.jsonl, val.jsonl, test.jsonl, manifest.json
```

Para regenerarlos:

```bash
uv run python scripts/01_generate_dataset.py --mode offline --count 1600
# con profesor:
uv run python scripts/01_generate_dataset.py --mode auto
```

Cada línea es un registro con:

- `messages`: system + user + assistant, listo para SFT.
- `label`: la decisión validada contra `schema.intent_schema`.
- `context`: proyectos/tareas que vio el router.
- `meta`: escenario, idioma, fuente (`offline`/`teacher`) y semilla.

El split es estratificado por intención. `manifest.json` guarda conteos,
distribución, fuentes y hashes.
