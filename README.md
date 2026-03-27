# robot_master <img width="25%" src="https://www.jeuxdenim.be/images/jeux/RobotMaster_large01.jpg" alt="Robot Master">
![Minimum Supported Rust Version](https://img.shields.io/badge/nightly-1.92+-ab6000.svg)
[<img alt="crates.io" src="https://img.shields.io/crates/v/robot_master.svg?color=fc8d62&logo=rust" height="20" style=flat-square>](https://crates.io/crates/robot_master)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs&style=flat-square" height="20">](https://docs.rs/robot_master)
![Lines Of Code](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/valeratrades/b48e6f02c61942200e7d1e3eeabf9bcb/raw/robot_master-loc.json)
<br>
[<img alt="ci errors" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master/errors.yml?branch=master&style=for-the-badge&style=flat-square&label=errors&labelColor=420d09" height="20">](https://github.com/valeratrades/robot_master/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->
[<img alt="ci warnings" src="https://img.shields.io/github/actions/workflow/status/valeratrades/robot_master/warnings.yml?branch=master&style=for-the-badge&style=flat-square&label=warnings&labelColor=d16002" height="20">](https://github.com/valeratrades/robot_master/actions?query=branch%3Amaster) <!--NB: Won't find it if repo is private-->

multi-player implementation of robot_master // in rust, because of course it is


### Reqs and `py_src/`
provisioned pdf with requirements: ./Sujet-RobotMaster-version-04-02.pdf

rough arch outline, functionality of each function, tests, desired behavior, - can be found in this pdf file

## Rules
1v1 on a 5x5 grid. Cards are numbered 0–5, with 6 copies each (36 total). Each player gets 12; a 25th card is placed at the center of the board.

**Turns**: players alternate placing a card from their hand onto an empty cell adjacent (no diagonals) to an occupied one.

**Scoring** (per line/column, once the grid is full):
| copies of a card | points |
|---|---|
| 1 | face value (0, 1, 2, 3, 4, or 5) |
| 2 | 10 × face value (0, 10, 20, 30, 40, or 50) |
| 3+ | 100 flat, regardless of face value |

**Winner**: Alice's score = her lowest-scoring column; Bob's score = his lowest-scoring row. Highest score wins.



<br>

<sup>
	This repository follows <a href="https://github.com/valeratrades/.github/tree/master/best_practices">my best practices</a> and <a href="https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md">Tiger Style</a> (except "proper capitalization for acronyms": (VsrState, not VSRState) and formatting). For project's architecture, see <a href="./docs/ARCHITECTURE.md">ARCHITECTURE.md</a>.
</sup>

#### License

<sup>
	Licensed under <a href="LICENSE">Blue Oak 1.0.0</a>
</sup>

<br>

<sub>
	Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be licensed as above, without any additional terms or conditions.
</sub>

