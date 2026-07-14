this proj compile .gsp to single html file

always based on .gsp file payload and .htm file, which has part of the geometry object definition, don't heuristic, dont add ad-hoc logic.

stick to htm file's definition. you can update current tests if they are wrong.

make sure the element is interactive and linked to needed calculation, check the payload carefully.

make sure .log contents contains .htm 's . .htm file's content may not complete.

Break compatibility if necessary.

Always clean the reduntant code.

refer to GSP5Chs.exe.c and GSP5Chs.exe.h for reference, if necessary you can rename some functions.

write as much logic into rust side, ts is used for generate bindings.


---

the whole step is

read in a gsp
parse into json
bundle json and runtime into single html

the json format is exported by rust struct.

---

after adding support, always add test for it.


the rust part is a huge DAG looks like

obj = op(parents...)

use a table driven style.
