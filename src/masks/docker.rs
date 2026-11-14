//! `docker compose up` (`masks/docker.py`): every paragraph is logged by one container.

use super::base::{Line, Segment, chrome, chrome_owned, progress_bar_label, seg};
use super::palette::{BLUE, CONTROL, CYAN, DIM, FG, GREEN, YELLOW};
use super::{Mask, Rng, pick, section};
use crate::book::{Paragraph, Pos};
use crate::text::ragged;
use unicode_width::UnicodeWidthStr;

const BOOK_SVC: &str = "worker-1";
/// (container, colour), in the order compose lists them when attaching.
const SERVICES: &[(&str, &str)] = &[
    ("api-1", YELLOW),
    ("db-1", BLUE),
    ("redis-1", CONTROL),
    ("web-1", GREEN),
    ("worker-1", CYAN),
];
const SVC_WIDTH: usize = 8;
/// Below this the service name is dropped, only the bar stays.
const WIDE: usize = 30;

fn messages(svc: &str) -> &'static [&'static str] {
    match svc {
        "api-1" => &[
            "INFO:     172.18.0.5:41234 - \"GET /health HTTP/1.1\" 200 OK",
            "INFO:     172.18.0.5:41250 - \"POST /v1/jobs HTTP/1.1\" 202 Accepted",
            "INFO:     Application startup complete.",
            "WARNING:  slow response from upstream (812 ms)",
        ],
        "db-1" => &[
            "LOG:  checkpoint starting: time",
            "LOG:  checkpoint complete: wrote 42 buffers (0.3%)",
            "LOG:  database system is ready to accept connections",
            "LOG:  autovacuum: processing table \"public.jobs\"",
        ],
        "web-1" => &[
            "172.18.0.1 - - [11/Nov/2026:09:14:02 +0000] \"GET / HTTP/1.1\" 200 612",
            "172.18.0.1 - - [11/Nov/2026:09:14:03 +0000] \"GET /static/app.js HTTP/1.1\" 304 0",
            "2026/11/11 09:14:05 [notice] 1#1: signal process started",
        ],
        "redis-1" => &[
            "1:M 11 Nov 2026 09:14:00.112 * Background saving started by pid 42",
            "1:M 11 Nov 2026 09:14:00.215 * DB saved on disk",
            "1:M 11 Nov 2026 09:14:00.311 * Ready to accept connections tcp",
        ],
        _ => &[
            "[tasks] job 4821 received",
            "[tasks] job 4821 succeeded in 0.84s",
            "[beat] scheduler: sending due task cleanup",
        ],
    }
}

fn prefix(svc: &str, colour: &str, width: usize) -> Segment {
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
pub struct DockerMask;

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

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        let (svc, colour) = pick(rng, SERVICES);
        chrome_owned(
            width,
            vec![
                prefix(svc, colour, width),
                seg(*pick(rng, messages(svc)), FG),
            ],
        )
    }

    fn line_count(&self, para: &Paragraph, width: usize) -> usize {
        ragged(&para.text, width.saturating_sub(prefix_len(width)).max(1)).len()
    }

    fn render(
        &mut self,
        para: &Paragraph,
        pos: Pos,
        width: usize,
        _rng: &mut Rng,
        _line_no_start: usize,
    ) -> Vec<Line> {
        ragged(&para.text, width.saturating_sub(prefix_len(width)).max(1))
            .into_iter()
            .map(|text| {
                Line::book(
                    vec![
                        prefix(BOOK_SVC, CYAN, width),
                        seg(text, format!("book {FG}")),
                    ],
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
        let lines = DockerMask.render(&para(), (0, 0), 80, &mut r, 1);
        assert!(lines.iter().all(|l| l.plain().starts_with("worker-1 | ")));
        assert!(DockerMask.noise(&mut r, 80).plain().contains(" | "));
        assert!(
            DockerMask.header(&mut r, 80)[1]
                .plain()
                .starts_with("Attaching to ")
        );
    }

    #[test]
    fn docker_narrow_pane_drops_the_service_name() {
        let lines = DockerMask.render(&para(), (0, 0), 25, &mut Rng::seed_from_u64(1), 1);
        assert!(
            lines
                .iter()
                .all(|l| l.plain().starts_with("| ") && l.plain().chars().count() <= 25)
        );
    }
}
