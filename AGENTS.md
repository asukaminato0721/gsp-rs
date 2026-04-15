this proj compile .gsp to single html file

always based on .gsp file payload, don't heuristic

make sure the element is interactive.

Break compatibility if necessary.

Always clean the reduntant code.

---

the whole step is

read in a gsp
parse into json
bundle json and runtime into single html

the json format is exported by rust struct.

---

after adding support, always add test for it.

---
