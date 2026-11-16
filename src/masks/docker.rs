//! `docker compose up` (`masks/docker.py`): every paragraph is logged by one container.

use super::base::{Line, Segment, chrome, chrome_owned, progress_bar_label, seg};
use super::filler::{Slot, docker_deck};
use super::look::Look;
use super::palette::{BLUE, CONTROL, CYAN, DIM, FG, GREEN, YELLOW};
use super::{Mask, Rng, pick, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};
use unicode_width::UnicodeWidthStr;

const BOOK_SVC: &str = "worker-1";
/// (container, colour), in the order compose lists them when attaching.
const SERVICES: &[(&str, Look)] = &[
    ("api-1", YELLOW),
    ("db-1", BLUE),
    ("redis-1", CONTROL),
    ("web-1", GREEN),
    ("worker-1", CYAN),
];
const SVC_WIDTH: usize = 8;
/// Below this the service name is dropped, only the bar stays.
const WIDE: usize = 30;

fn prefix(svc: &str, colour: Look, width: usize) -> Segment {
    if width >= WIDE {
        seg(format!("{svc:<SVC_WIDTH$} | "), colour)
    } else {
        seg("| ", colour)
    }
}

fn prefix_len(width: usize) -> usize {
    prefix(BOOK_SVC, CYAN, width).0.width()
}

#[derive(Default)]
pub struct DockerMask {
    slot: Slot,
}

impl Mask for DockerMask {
    fn name(&self) -> &'static str {
        "docker"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        let names: Vec<&str> = SERVICES.iter().map(|(s, _)| *s).collect();
        vec![
            chrome(width, &[("[+] Running 5/5", FG)]),
            chrome(
                width,
                &[(&format!("Attaching to {}", names.join(", ")), DIM)],
            ),
        ]
    }

    fn stream(&mut self) {
        self.slot.stream();
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        let (svc, colour) = pick(rng, SERVICES);
        chrome_owned(
            width,
            vec![
                prefix(svc, *colour, width),
                seg(docker_deck(svc).get(self.slot.take()), FG),
            ],
        )
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged_count(&para.text, width.saturating_sub(prefix_len(width)).max(1))
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        line_no_start: usize,
    ) -> Vec<Line> {
        self.slot.set(line_no_start);
        ragged(&para.text, width.saturating_sub(prefix_len(width)).max(1))
            .into_iter()
            .map(|text| {
                Line::book(
                    vec![prefix(BOOK_SVC, CYAN, width), seg(text, FG.book())],
                    pos,
                )
            })
            .collect()
    }

    /// The worker container restarting.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let name = section::name(chapter, 0);
        let code = section::choose(&[0, 0, 137, 0, 1], chapter);
        let start = match chapter % 3 {
            0 => format!("[entrypoint] starting worker queue={name}"),
            1 => format!("[tasks] worker ready, consuming from {name}"),
            _ => format!("[entrypoint] migrations applied, serving {name}"),
        };
        vec![
            chrome(
                width,
                &[(&format!("{BOOK_SVC} exited with code {code}"), DIM)],
            ),
            chrome_owned(width, vec![prefix(BOOK_SVC, CYAN, width), seg(start, FG)]),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Extracting")
    }

    fn status_right(
        &self,
        done: usize,
        total: usize,
        elapsed: std::time::Duration,
    ) -> Vec<super::base::Segment> {
        let _ = (done, total, elapsed);
        vec![
            super::base::seg("● 5/5 running", super::palette::GREEN),
            super::base::seg("  ·  ", super::palette::DIM),
            super::base::seg("compose: dev", super::palette::BLUE),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn para() -> Paragraph {
        Paragraph::new("Sabah erkenden uyandı ve pencereyi açtı. ".repeat(4))
    }

    // -- test_git_docker.py (docker) --

    #[test]
    fn docker_lines_carry_a_container_prefix() {
        let mut r = Rng::seed_from_u64(1);
        let lines = DockerMask::default().render(&para(), (0, 0), 80, &mut r, 1);
        assert!(lines.iter().all(|l| l.plain().starts_with("worker-1 | ")));
        assert!(
            DockerMask::default()
                .noise(&mut r, 80)
                .plain()
                .contains(" | ")
        );
        assert!(
            DockerMask::default().header(&mut r, 80)[1]
                .plain()
                .starts_with("Attaching to ")
        );
    }

    #[test]
    fn docker_narrow_pane_drops_the_service_name() {
        let lines =
            DockerMask::default().render(&para(), (0, 0), 25, &mut Rng::seed_from_u64(1), 1);
        assert!(
            lines
                .iter()
                .all(|l| l.plain().starts_with("| ") && l.plain().chars().count() <= 25)
        );
    }
}
