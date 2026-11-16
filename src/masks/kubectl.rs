//! `kubectl logs -f --prefix` over a namespace: every paragraph is logged by one pod,
//! the other pods' lines are filler, and a new chapter is the pod's container restarting.

use std::sync::OnceLock;

use super::base::{Line, Segment, chrome, chrome_owned, progress_bar_label, seg};
use super::codegen::Deck;
use super::filler::{Slot, expand, seq, words};
use super::look::Look;
use super::palette::{BLUE, CONTROL, CYAN, DIM, FG, GREEN, YELLOW};
use super::{Mask, Rng, pick, section};
use crate::book::{Paragraph, Pos};
use crate::text::{ragged, ragged_count};
use unicode_width::UnicodeWidthStr;

const BOOK_POD: &str = "worker-5f7c9-x2kqp";
/// (pod, colour), in the order `kubectl get pods` lists them.
const PODS: &[(&str, Look)] = &[
    ("api-6d4b9c-qq2lm", YELLOW),
    ("cron-5c9d7-h7r2p", GREEN),
    ("ingress-7f8d-zk4tx", BLUE),
    ("redis-0", CONTROL),
    ("worker-5f7c9-x2kqp", CYAN),
];
const POD_WIDTH: usize = 18;
/// Below this the pod name is dropped, only the bar stays.
const WIDE: usize = 60;
const NAMESPACE: &str = "prod";

fn prefix(pod: &str, colour: Look, width: usize) -> Segment {
    if width >= WIDE {
        seg(format!("[{pod:<POD_WIDTH$}] "), colour)
    } else {
        seg("| ", colour)
    }
}

fn prefix_len(width: usize) -> usize {
    prefix(BOOK_POD, CYAN, width).0.width()
}

// -- filler ----------------------------------------------------------------------

/// Minutes or seconds of a clock: `00` to `59`.
fn two_digits() -> Vec<String> {
    (0..60).map(|i| format!("{i:02}")).collect()
}

fn vocab() -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "res",
            words(&[
                "users", "orders", "jobs", "sessions", "invoices", "tokens", "events", "reports",
                "uploads", "carts", "webhooks", "exports", "devices", "tenants",
            ]),
        ),
        ("ms", seq(2, 13, 40)),
        ("n", seq(14, 23, 50)),
        ("id", seq(7301, 17, 60)),
        ("k", seq(1, 1, 5)),
        ("ip", seq(3, 7, 30)),
        ("bytes", seq(312, 287, 40)),
        ("mm", two_digits()),
        ("ss", two_digits()),
        ("us", seq(104_211, 31_337, 24)),
        ("line", seq(88, 13, 20)),
        ("msec", seq(104, 37, 24)),
        ("st", words(&["200", "201", "204", "304", "200", "200"])),
        ("method", words(&["GET", "POST", "PUT", "GET", "PATCH"])),
        (
            "ua",
            words(&[
                "kube-probe/1.31",
                "Go-http-client/2.0",
                "okhttp/4.12.0",
                "python-httpx/0.27.2",
                "curl/8.9.1",
            ]),
        ),
        (
            "task",
            words(&[
                "cleanup",
                "reindex",
                "send_digest",
                "expire_sessions",
                "rotate_keys",
                "sync_billing",
                "prune_exports",
                "refresh_cache",
                "compact_events",
                "renew_certs",
            ]),
        ),
        (
            "cmd",
            words(&[
                "GET", "SET", "EXPIRE", "HGETALL", "ZADD", "LPUSH", "DEL", "INCR",
            ]),
        ),
    ]
}

const API: &[&str] = &[
    "level=info msg=\"request served\" method={method} path=/v1/{res} status={st} dur={ms}ms",
    "level=info msg=\"request served\" method=GET path=/v1/{res}/{id} status={st} dur={ms}ms",
    "level=debug msg=\"cache lookup\" key={res}:{id} hit=true",
    "level=info msg=\"token verified\" sub=user:{id} scope={res}:read",
    "level=warn msg=\"slow upstream\" svc={res}-svc dur={ms}ms attempt={k}",
    "level=info msg=\"pool stats\" open={n} idle={k} waiting=0",
];
const CRON: &[&str] = &[
    "I1116 09:{mm}:{ss}.{us}       1 controller.go:{line}] synced job {task}-{id}",
    "I1116 09:{mm}:{ss}.{us}       1 scheduler.go:{line}] next run of {task} in {n}s",
    "I1116 09:{mm}:{ss}.{us}       1 leader.go:{line}] lease renewed holder=cron-5c9d7-h7r2p",
    "W1116 09:{mm}:{ss}.{us}       1 controller.go:{line}] job {task}-{id} took {ms}ms, over budget",
];
const INGRESS: &[&str] = &[
    "10.42.{k}.{ip} - - [16/Nov/2026:09:{mm}:{ss} +0000] \"{method} /v1/{res} HTTP/2.0\" {st} {bytes} \"-\" \"{ua}\" 0.0{ms} [prod-api-80]",
    "10.42.{k}.{ip} - - [16/Nov/2026:09:{mm}:{ss} +0000] \"GET /v1/{res}/{id} HTTP/2.0\" {st} {bytes} \"-\" \"{ua}\"",
    "10.42.{k}.{ip} - - [16/Nov/2026:09:{mm}:{ss} +0000] \"GET /healthz HTTP/1.1\" 200 2 \"-\" \"{ua}\" 0.00{k}",
];
const REDIS: &[&str] = &[
    "1:M 16 Nov 2026 09:{mm}:{ss}.{msec} * {n} changes in 60 seconds. Saving...",
    "1:M 16 Nov 2026 09:{mm}:{ss}.{msec} * Background saving started by pid {id}",
    "1:M 16 Nov 2026 09:{mm}:{ss}.{msec} * Background saving terminated with success",
    "1:M 16 Nov 2026 09:{mm}:{ss}.{msec} - Accepted 10.42.{k}.{ip}:{bytes}",
    "1:M 16 Nov 2026 09:{mm}:{ss}.{msec} # slowlog: {cmd} {res}:{id} took {ms}00us",
];
const WORKER: &[&str] = &[
    "level=info msg=\"task received\" task={task} id={id}",
    "level=info msg=\"task done\" task={task} id={id} took={ms}ms",
    "level=info msg=\"batch flushed\" queue={res} size={n}",
    "level=warn msg=\"task retry\" task={task} id={id} attempt={k}/5",
    "level=debug msg=\"lease extended\" queue={res} id={id}",
];

macro_rules! pool {
    ($name:ident, $templates:expr, $cap:literal, $seed:literal) => {
        fn $name() -> &'static Deck {
            static D: OnceLock<Deck> = OnceLock::new();
            D.get_or_init(|| Deck::new(expand($templates, &vocab(), $cap), $seed))
        }
    };
}

pool!(api_deck, API, 120, "kubectl-api");
pool!(cron_deck, CRON, 150, "kubectl-cron");
pool!(ingress_deck, INGRESS, 200, "kubectl-ingress");
pool!(redis_deck, REDIS, 120, "kubectl-redis");
pool!(worker_deck, WORKER, 120, "kubectl-worker");

/// The pool of a pod's log.
fn deck(pod: &str) -> &'static Deck {
    match pod {
        "api-6d4b9c-qq2lm" => api_deck(),
        "cron-5c9d7-h7r2p" => cron_deck(),
        "ingress-7f8d-zk4tx" => ingress_deck(),
        "redis-0" => redis_deck(),
        _ => worker_deck(),
    }
}

/// A row of `kubectl get pods`.
fn pod_row(name: &str, ready: &str, status: &str, restarts: usize, age: &str) -> String {
    format!("{name:<20} {ready:<7} {status:<17} {restarts:<10} {age}")
}

#[derive(Default)]
pub struct KubectlMask {
    slot: Slot,
}

impl Mask for KubectlMask {
    fn name(&self) -> &'static str {
        "kubectl"
    }

    fn header(&mut self, _rng: &mut Rng, width: usize) -> Vec<Line> {
        let ages = ["3d4h", "3d4h", "11d", "27d", "41m"];
        let mut out = vec![
            chrome(
                width,
                &[
                    ("$ ", DIM),
                    (&format!("kubectl get pods -n {NAMESPACE}"), FG),
                ],
            ),
            chrome(
                width,
                &[(
                    &format!(
                        "{:<20} {:<7} {:<17} {:<10} AGE",
                        "NAME", "READY", "STATUS", "RESTARTS"
                    ),
                    DIM,
                )],
            ),
        ];
        for ((pod, _), age) in PODS.iter().zip(ages) {
            let restarts = usize::from(*pod == BOOK_POD) * 2;
            out.push(chrome(
                width,
                &[(&pod_row(pod, "1/1", "Running", restarts, age), FG)],
            ));
        }
        out.push(chrome(
            width,
            &[
                ("$ ", DIM),
                (
                    &format!("kubectl logs -f -n {NAMESPACE} -l tier=backend --prefix --tail=20"),
                    FG,
                ),
            ],
        ));
        out
    }

    fn stream(&mut self) {
        self.slot.stream();
    }

    fn noise(&mut self, rng: &mut Rng, width: usize) -> Line {
        let (pod, colour) = pick(rng, PODS);
        chrome_owned(
            width,
            vec![
                prefix(pod, *colour, width),
                seg(deck(pod).get(self.slot.take()), FG),
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
                    vec![prefix(BOOK_POD, CYAN, width), seg(text, FG.book())],
                    pos,
                )
            })
            .collect()
    }

    /// The worker's container restarting, as `kubectl get pods -w` shows it.
    fn transition(&self, chapter: usize, width: usize, _line_no_start: usize) -> Vec<Line> {
        let name = section::name(chapter, 0);
        let restarts = 2 + chapter;
        let (status, code): (&str, u8) = section::choose(
            &[
                ("Completed", 0),
                ("Completed", 0),
                ("OOMKilled", 137),
                ("Error", 1),
            ],
            chapter,
        );
        let age = format!("{}m", 41 + chapter * 7);
        let start = match chapter % 3 {
            0 => format!("level=info msg=\"consumer started\" queue={name}"),
            1 => format!("level=info msg=\"worker ready\" queue={name} concurrency=8"),
            _ => format!("level=info msg=\"migrations applied\" serving={name}"),
        };
        let down = pod_row(BOOK_POD, "0/1", status, restarts - 1, &age);
        vec![
            chrome(width, &[(&down, if code == 0 { DIM } else { YELLOW })]),
            chrome(
                width,
                &[(&pod_row(BOOK_POD, "1/1", "Running", restarts, &age), DIM)],
            ),
            chrome_owned(width, vec![prefix(BOOK_POD, CYAN, width), seg(start, FG)]),
        ]
    }

    fn progress_label(&self, done: usize, total: usize, _width: usize) -> String {
        progress_bar_label(done, total, 10, "Streaming")
    }

    fn status_right(
        &self,
        _done: usize,
        _total: usize,
        _elapsed: std::time::Duration,
    ) -> Vec<Segment> {
        vec![
            seg("⎈ prod-eu-1", CYAN),
            seg("  ·  ", DIM),
            seg(format!("ns: {NAMESPACE}"), BLUE),
            seg("  ·  ", DIM),
            seg("5/5 ready", GREEN),
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

    #[test]
    fn header_lists_pods_then_follows_their_logs() {
        let text: Vec<String> = KubectlMask::default()
            .header(&mut Rng::seed_from_u64(1), 120)
            .iter()
            .map(Line::plain)
            .collect();
        assert_eq!(text[0], "$ kubectl get pods -n prod");
        assert!(text[1].starts_with("NAME ") && text[1].ends_with("AGE"));
        assert!(
            PODS.iter()
                .all(|(p, _)| text.iter().any(|l| l.starts_with(p)))
        );
        assert!(text.last().unwrap().starts_with("$ kubectl logs -f"));
    }

    #[test]
    fn text_lines_carry_the_worker_pod_prefix() {
        let mut r = Rng::seed_from_u64(1);
        let lines = KubectlMask::default().render(&para(), (0, 0), 100, &mut r, 1);
        assert!(
            lines
                .iter()
                .all(|l| l.plain().starts_with("[worker-5f7c9-x2kqp] "))
        );
        let n = KubectlMask::default().noise(&mut r, 100).plain();
        assert!(n.starts_with('[') && n.contains("] "), "{n}");
    }

    #[test]
    fn narrow_pane_drops_the_pod_name() {
        let lines =
            KubectlMask::default().render(&para(), (0, 0), 40, &mut Rng::seed_from_u64(1), 1);
        assert!(lines.iter().all(|l| l.plain().starts_with("| ")));
    }

    #[test]
    fn pools_are_large_and_clean() {
        for d in [
            api_deck(),
            cron_deck(),
            ingress_deck(),
            redis_deck(),
            worker_deck(),
        ] {
            assert!(d.len() >= 400, "{}", d.len());
            for i in 0..d.len() {
                let low = d.get(i).to_lowercase();
                assert!(!low.contains("book") && !low.contains("reader"), "{low}");
            }
        }
    }
}
