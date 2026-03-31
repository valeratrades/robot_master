TODO: apparently need this to be able to submit here https://moodle2025.uca.fr/mod/assign/view.php?id=518337

reqs: read ./Sujet-RobotMaster-version-04-02.pdf (ignore the Latex part, - we're using typst)

NB: don't forget to mention:
- link to and state at each tag (last python-only version was v0.2.0)
- full description of functionality
- don't restate things in docs/ like ARCHITECTURE.md - just point to them (full links on gitlab (master))
  TODO: talking ARCHITECTURE.md, - just noticed it's missing. Need to fill in before even starting with this
- extra attention to all the things we changed from the original, like
  - aggressive tests in /home/v/uni/robot_master/py_src/IA/IA_test.py to be based on final score (cause tiebrake rules shouldn't influence)
  - type in some tests like /home/v/uni/robot_master/py_src/partie_guidee/c_test.py:65 are switched to ones that make more sense
    // here it's a tuple for `("B", "r", {3: 1})`, not an array (cause first two are positional)
  - mention I didn't know abt `IA_test.py` for a while, so it won't exist or pass until `v0.5.0`
- all the links are to `gitlab` only, - github of this project is private (yet is source of truth).
- include the current leaderboard in the submission (run `arena players list`)

NB: when talking about fundamental design decisions and things that should go into docs/, - just put them in docs, then link. Despite what they say in the pdf file, we're writing more of a progress report, and actually important things should be persisted alongside the code itself not in some random compiled pdf.
