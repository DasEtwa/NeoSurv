# OBJ Model Import (Game Slice)

Diese Implementierung ist bewusst ein kleiner Game-Vertical-Slice und keine allgemeine Engine-Asset-Pipeline.

## Aktuell unterstuetzt

- `.obj` als erstes Modellformat
- triangulierte Meshes via `tobj`
- Vertex-Positionen
- Indices
- UVs
- Normals
- optionale `.mtl`-Dateien
- `.mtl` Diffuse-Farbe (`Kd`) wird als Basisfarbe ins Rendering uebernommen

## Aktuell nicht unterstuetzt

- komplexe MTL-Features wie Specular-, Roughness-, Emission- oder Transparenz-Workflows
- Material-Parameter ausser Diffuse-Farbe
- Textur-Upload und Textur-Sampling fuer OBJ-Modelle
- Skelettanimationen
- dynamische Runtime-Instancing-API
- Modellrotation/Transform per GPU-Uniform

Wenn eine `.mtl` referenzierte Texturen nennt, werden diese aktuell nur erkannt und geloggt. Das Rendering bleibt im ersten Schritt Mesh-only plus Materialfarbe.

## Test-Asset

Der erste Testpfad liegt projektintern unter:

- `assets/models/pistol_1/Pistol_1.obj`
- `assets/models/pistol_1/Pistol_1.mtl`

Quelle dafuer war initial:

- `Z:\workspace\game-engine\Ressourcen\OBJ`

Damit ist die Runtime nicht an einen absoluten lokalen Pfad gebunden.

## Runtime-Nutzung

Der erste harte Spawn passiert aktuell in [engine.rs](/Z:/workspace/NeoSurv/src/engine.rs).

Verwendet wird:

- `assets/models/pistol_1/Pistol_1.obj`
- feste Weltposition vor dem Spawnbereich
- fester Uniform-Scale-Wert

Der Ladepfad laeuft ueber [model.rs](/Z:/workspace/NeoSurv/src/game/model.rs) und der Upload/Draw-Pfad ueber [mod.rs](/Z:/workspace/NeoSurv/src/renderer/mod.rs).

## Neue Modelle ablegen

Neue OBJ-Modelle bitte so ablegen:

1. Unter `assets/models/<dein_modell>/`
2. `.obj` und optionale `.mtl` in denselben Ordner
3. referenzierte Texturen spaeter ebenfalls in denselben Ordner, damit relative Materialpfade stabil bleiben

## Neue Modelle nutzen

Fuer den aktuellen Stand reicht:

1. Pfad in `src/engine.rs` auf das neue `.obj` setzen
2. Spawn-Position und Scale anpassen
3. Spiel starten

## Praktische Einschraenkungen

- Der Spawn ist aktuell hart codiert.
- Modelltransforms werden zur Zeit CPU-seitig in die Vertex-Positionen eingebacken.
- Mehrere Materialien innerhalb einer OBJ werden als mehrere Teilmeshes geladen und gemeinsam gerendert.
- Fehlende UVs oder Normals brechen den Import nicht; es werden Defaultwerte verwendet.
