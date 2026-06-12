The editor under test mixes paragraph text with painted list markers, and that mix is exactly the trigger class for the Wayland scale-flip corruption.

- First bullet item with enough words to wrap once the column narrows on a scale change, включая кириллицу для давления на атлас.
- Second bullet item: glyph variety helps — fi fl ffi, “quotes”, 1941—1945.

A trailing paragraph after the list keeps the WrappedLine paints interleaved with the marker ShapedLine paints, which is the bisected trigger.

Ещё один абзац по-русски, чтобы оба алфавита были на экране одновременно и любые спрайты неправильного размера бросались в глаза.
