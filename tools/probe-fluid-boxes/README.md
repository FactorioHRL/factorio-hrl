# probe-fluid-boxes

Mały mod do Factorio 2.1, który stawia maszyny w różnych orientacjach, z odbiciem (`mirror`) i recepturami, a potem zapisuje do `script-output/probe.json` połączenia rurowe, które **zgłasza sama gra** (`LuaEntity::get_fluid_box_pipe_connections`). Służy do sprawdzania, czy nasza logika połączeń (`crates/fsim-render/src/fluids.rs`) zgadza się z silnikiem. Oczekiwane wartości w testach tego modułu pochodzą z jego wyników.

Sonda stawia: chemical plant (receptury `plastic-bar`, `light-oil-cracking` i bez receptury, 4 kierunki, z odbiciem i bez), rafinerię (`basic-oil-processing`), pipe-to-ground i zbiornik.

## Uruchomienie (WSL, instalacja Steam na Windows)

```
P=/mnt/c/temp/fsim-probe
mkdir -p $P/config $P/mods/fsim-probe_0.0.1
cp tools/probe-fluid-boxes/mod/* $P/mods/fsim-probe_0.0.1/
cp tools/probe-fluid-boxes/mod-list.json $P/mods/
printf '[path]\nread-data=__PATH__executable__/../../data\nwrite-data=C:/temp/fsim-probe\n' > $P/config/config.ini

cd /mnt/c/temp
"/mnt/c/Program Files (x86)/Steam/steamapps/common/Factorio/bin/x64/factorio.exe" \
  --config C:/temp/fsim-probe/config/config.ini \
  --mod-directory C:/temp/fsim-probe/mods \
  --create C:/temp/fsim-probe/probe.zip
```

Wynik: `$P/script-output/probe.json`. Mod wykonuje się w `on_init` podczas tworzenia mapy, więc nie trzeba uruchamiać serwera.

## Czego dowiedzieliśmy się z sondy (Factorio 2.1.21)

- `mirror` odbija porty maszyny względem jej osi X (przed obrotem).
- Receptura z **jedną** cieczą po danej stronie (wejście albo wyjście) łączy **wszystkie** porty tej strony w jeden zbiornik, więc chemical plant z `plastic-bar` ma oba porty wejścia aktywne.
- Z dwiema cieczami każdy port ma swoją ciecz (kolejno).
- Gdy w recepturze jest `fluidbox_index`, wskazuje on port (numeracja od 1 wśród portów tego typu). Rafineria z `basic-oil-processing` używa wejścia nr 2 i wyjścia nr 3.
- Receptura bez cieczy po danej stronie wyłącza porty tej strony.
- Maszyna bez receptury ma wszystkie porty aktywne (chyba że prototyp ustawia `fluid_boxes_off_when_no_fluid_recipe`).
- Pipe-to-ground ma jedno połączenie zwykłe (z tyłu względem kierunku podziemnego) i jedno `underground`.
