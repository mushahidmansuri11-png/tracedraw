# TraceDraw fixes - GitHub web se kaise add karein

Chrome mein GitHub **Desktop site** on karo (menu > Desktop site), edit karna aasaan hoga.

## Part 1: 19 lang files (sabme same 12 lines, file ke END mein)

`lang_append.txt` kholo, saari lines copy karo. Phir har file ke liye: GitHub par file kholo > pencil (Edit) icon > sabse neeche jao > naye line par paste karo > Commit.

Folder: `apps/tracedraw/lang/`

- ar.toml
- bn.toml
- cs.toml
- de.toml
- en.toml
- es.toml
- fr.toml
- hi.toml
- id.toml
- it.toml
- ja.toml
- lo.toml
- nl.toml
- pl.toml
- pt-BR.toml
- ru.toml
- tr.toml
- zh-CN.toml
- zh-TW.toml

## Part 2: 8 Rust files (har ek ka alag .diff file)

Diff padhna: `-` wali line **hatao**, `+` wali line **jodo**, bina `+/-` wali line sirf jagah dhoondhne ke liye (context) hai. `@@ -123 ...` ka number line number ka andaza hai, main ke naye commits se thoda aage-peeche ho sakta hai.

| File (GitHub path) | Diff file | Hunks | + | - |
|---|---|---|---|---|
| `apps/tracedraw/src/app.rs` | `apps__tracedraw__src__app.rs.diff` | 2 | 4 | 0 |
| `apps/tracedraw/src/eyedropper.rs` | `apps__tracedraw__src__eyedropper.rs.diff` | 1 | 18 | 1 |
| `apps/tracedraw/src/ops2.rs` | `apps__tracedraw__src__ops2.rs.diff` | 2 | 19 | 3 |
| `apps/tracedraw/src/ui/color_docker.rs` | `apps__tracedraw__src__ui__color_docker.rs.diff` | 1 | 20 | 6 |
| `apps/tracedraw/src/ui/toolbar.rs` | `apps__tracedraw__src__ui__toolbar.rs.diff` | 1 | 7 | 0 |
| `apps/tracedraw/src/ui/tracecut_cut.rs` | `apps__tracedraw__src__ui__tracecut_cut.rs.diff` | 2 | 74 | 0 |
| `apps/tracedraw/src/ui/transform_docker.rs` | `apps__tracedraw__src__ui__transform_docker.rs.diff` | 2 | 10 | 0 |
| `crates/tracecut/src/cnc.rs` | `crates__tracecut__src__cnc.rs.diff` | 7 | 142 | 0 |

Sabse bada: `crates/tracecut/src/cnc.rs` (7 jagah, 142 lines). Sabse aakhir mein karo.

## Zaroori
Agar kisi jagah context line main mein mil hi nahi rahi, matlab main us file mein badal chuka hai. Wahan ruk ke mujhe batao.
