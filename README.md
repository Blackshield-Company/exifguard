# exifguard

Batch forensic verifier for evidence photos. exifguard scans JPEG files and flags signs of
editing or tampering by analyzing EXIF metadata — stripped metadata, editing-software tags,
impossible timestamps, missing camera identity, and stale embedded thumbnails.

**Local-first.** No network access, no uploads, nothing leaves your machine.

## Who it's for

Anyone who needs to assess whether a photo is a plausible camera original or has been
through an editor:

- **Defense teams** challenging the authenticity of photographic evidence.
- **Prosecutors and investigators** sanity-checking their own exhibits before disclosure.
- Journalists, archivists, insurance investigators, and anyone handed a "photo" and asked
  to trust it.

exifguard is deliberately symmetric: the same flags that support a tampering claim can
exonerate a file with boring, consistent metadata.

## Install

```sh
cargo install --path .
```

## Usage

```sh
# Recursively scan a directory of JPEGs, print a report table
exifguard scan ./evidence_photos

# Detailed report for a single file
exifguard check ./evidence_photos/IMG_0042.jpg

# Machine-readable output (both commands)
exifguard scan ./evidence_photos --json
exifguard check ./evidence_photos/IMG_0042.jpg --json
```

Example `scan` output:

```
PATH                     FLAGS  CODES
photos/IMG_0042.jpg          2  SOFTWARE_TAG, THUMBNAIL_MISMATCH
photos/IMG_0043.jpg          0  clean
photos/download.jpg          1  NO_EXIF

3 file(s) scanned, 2 flagged.
Flags are leads, not proof — run `exifguard check <file>` for details.
```

## What it flags

| Code | Meaning |
| --- | --- |
| `NO_EXIF` | No EXIF metadata at all — suspicious on a claimed camera original. |
| `SOFTWARE_TAG` | A Software tag (e.g. Photoshop, GIMP) shows the file was saved by software after capture. |
| `MISSING_DATETIME_ORIGINAL` | The capture timestamp is absent or unparseable. |
| `DATETIME_MTIME_MISMATCH` | EXIF claims the photo was taken *after* the file on disk was last written — impossible for an untouched original. |
| `MISSING_CAMERA_INFO` | Camera Make and/or Model missing on a claimed original. |
| `THUMBNAIL_MISMATCH` | The embedded preview thumbnail's aspect ratio doesn't match the main image — a classic sign the image was edited while the thumbnail (or vice versa) was left behind. |

Every finding in the output includes a plain-language explanation of why it matters.

## Honest limits

**Flags are leads, not proof.** exifguard answers "does this metadata look like an
untouched camera original?", not "was this photo manipulated?" Please keep in mind:

- **EXIF is stripped legitimately all the time.** Messaging apps, social media, email
  clients, and privacy tools remove metadata. `NO_EXIF` on a photo that came from WhatsApp
  is expected, not incriminating.
- **Flags can have innocent causes.** A `SOFTWARE_TAG` may mean a benign crop or rotate.
  Re-saving a file to reduce its size rewrites metadata.
- **Absence of flags proves nothing.** A careful forger can strip or rewrite EXIF to look
  pristine. A "clean" result means only that these checks found no anomalies.
- **File mtime is fragile.** Copying a file updates mtime; that's why exifguard only flags
  the *impossible* direction (capture after modification), never the normal one.
- **EXIF analysis cannot detect content manipulation** in a well-prepared file. Pixel-level
  forensics (ELA, noise analysis, copy-move detection) is a separate discipline and out of
  scope.

Use exifguard findings to decide where to look closer — never as the sole basis for a
claim in either direction.

## License

Apache-2.0. See [LICENSE](LICENSE).

Made by synth with blackclaw
