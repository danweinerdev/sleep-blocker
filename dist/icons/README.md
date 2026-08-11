# Tray icons

`sleep-block-active.svg` and `sleep-block-idle.svg` are the sources; the PNGs
are generated from them and are what the binary embeds.

## Regenerating

```sh
cd dist/icons
for s in active idle; do
  for sz in 16 22 24 32 48 64 128 256; do
    magick -background none sleep-block-$s.svg -resize ${sz}x${sz} \
      -depth 8 -define png:color-type=6 -define png:bit-depth=8 \
      PNG32:sleep-block-$s-$sz.png
  done
done
```

**The 8-bit flags are required.** ImageMagick writes 16-bit PNGs by default,
and the tray decoder accepts only 8-bit RGBA — anything else is rejected and
the tray publishes an empty pixmap. The failure is silent: the app runs, the
inhibitor works, and the icon is simply missing. `cargo test --test icons`
guards against this.

## Design constraints

Both states share one silhouette so the app stays recognisable, and differ by
colour and saturation: warm amber-to-rose when blocking sleep, cool slate when
idle.

Panel themes are not knowable from inside the app, so both icons must hold
contrast against light *and* dark backgrounds. An earlier idle icon used a
thin, 75%-opacity stroke and became nearly invisible on a dark panel at 22px —
hence the full-weight stroke and mid-range slate tones. Check any change at
22px against both backgrounds before committing it.
