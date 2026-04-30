this proj compile .gsp to single html file

always based on .gsp file payload and .htm file, which has part of the geometry object definition, don't heuristic

if possible, stick to htm file's definition. you can update current tests if they are wrong.

make sure the element is interactive.

Break compatibility if necessary.

Always clean the reduntant code.

refer to GSP5Chs.exe.c and GSP5Chs.exe.h for reference, if necessary you can rename some functions.

---

the whole step is

read in a gsp
parse into json
bundle json and runtime into single html

the json format is exported by rust struct.

---

after adding support, always add test for it.

---
