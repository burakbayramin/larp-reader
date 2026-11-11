//! Filler source code per language for the code mask (`masks/snippets.py`).
//!
//! HEADERS open the file, ITEMS are one-line declarations that sit under a doc
//! comment, BLOCKS are complete multi-line definitions. Generated from the
//! Python module; keep the two in step.

pub fn header(lang: &str) -> &'static str {
    match lang {
        "python" => HEADER_PYTHON,
        "typescript" => HEADER_TYPESCRIPT,
        "go" => HEADER_GO,
        "rust" => HEADER_RUST,
        "java" => HEADER_JAVA,
        "csharp" => HEADER_CSHARP,
        _ => HEADER_PYTHON,
    }
}

pub fn items(lang: &str) -> &'static [&'static str] {
    match lang {
        "python" => ITEMS_PYTHON,
        "typescript" => ITEMS_TYPESCRIPT,
        "go" => ITEMS_GO,
        "rust" => ITEMS_RUST,
        "java" => ITEMS_JAVA,
        "csharp" => ITEMS_CSHARP,
        _ => ITEMS_PYTHON,
    }
}

pub fn blocks(lang: &str) -> &'static [&'static str] {
    match lang {
        "python" => BLOCKS_PYTHON,
        "typescript" => BLOCKS_TYPESCRIPT,
        "go" => BLOCKS_GO,
        "rust" => BLOCKS_RUST,
        "java" => BLOCKS_JAVA,
        "csharp" => BLOCKS_CSHARP,
        _ => BLOCKS_PYTHON,
    }
}

const HEADER_PYTHON: &str = r#"from __future__ import annotations

import logging
from dataclasses import dataclass
from pathlib import Path"#;
const HEADER_TYPESCRIPT: &str = r#"import { createHash } from "crypto";
import path from "path";

import type { Config, Span } from "./types";"#;
const HEADER_GO: &str = r#"package indexer

import (
    "errors"
    "sync"
)"#;
const HEADER_RUST: &str = r#"use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{Error, Result};"#;
const HEADER_JAVA: &str = r#"package com.example.indexer;

import java.util.HashMap;
import java.util.Map;

public final class Indexer {"#;
const HEADER_CSHARP: &str = r#"using System;
using System.Collections.Generic;

namespace Example.Indexing;

public sealed class Indexer
{"#;

const ITEMS_PYTHON: &[&str] = &[
    r#"MAX_CHUNK_SIZE = 4096"#,
    r#"DEFAULT_TIMEOUT = 30.0"#,
    r#"RETRY_LIMIT = 5"#,
    r#"logger = logging.getLogger(__name__)"#,
    r#"CACHE_DIR = Path.home() / ".cache" / "indexer""#,
    r#"_registry: dict[str, type] = {}"#,
    r#"ENCODINGS = ("utf-8", "cp1254", "latin-1")"#,
    r#"Handler = Callable[[bytes], bytes]"#,
];

const ITEMS_TYPESCRIPT: &[&str] = &[
    r#"export const MAX_CHUNK_SIZE = 4096;"#,
    r#"export const DEFAULT_TIMEOUT_MS = 30_000;"#,
    r#"const RETRY_LIMIT = 5;"#,
    r#"export type Handler = (data: Buffer) => Buffer;"#,
    r#"const registry = new Map<string, Handler>();"#,
    r#"export const ENCODINGS = ["utf-8", "latin1"] as const;"#,
    r#"let workerCount = 0;"#,
    r#"export type Cache = Map<string, Uint8Array>;"#,
];

const ITEMS_GO: &[&str] = &[
    r#"const MaxChunkSize = 4096"#,
    r#"const DefaultTimeout = 30 * time.Second"#,
    r#"const retryLimit = 5"#,
    r#"var ErrExhausted = errors.New("retries exhausted")"#,
    r#"var workerCount atomic.Int64"#,
    r#"type Handler func(data []byte) []byte"#,
    r#"type Cache map[string][]byte"#,
    r#"var registry = map[string]Handler{}"#,
];

const ITEMS_RUST: &[&str] = &[
    r#"pub const MAX_CHUNK_SIZE: usize = 4096;"#,
    r#"pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;"#,
    r#"const RETRY_LIMIT: u32 = 5;"#,
    r#"pub type Shared<T> = Arc<RwLock<T>>;"#,
    r#"pub struct Token(pub u64);"#,
    r#"static WORKER_COUNT: AtomicUsize = AtomicUsize::new(0);"#,
    r#"pub struct Span { pub start: usize, pub end: usize }"#,
    r#"type Cache = HashMap<String, Vec<u8>>;"#,
];

const ITEMS_JAVA: &[&str] = &[
    r#"    public static final int MAX_CHUNK_SIZE = 4096;"#,
    r#"    private static final long DEFAULT_TIMEOUT_MS = 30_000L;"#,
    r#"    private static final int RETRY_LIMIT = 5;"#,
    r#"    private final Map<String, Span> entries = new HashMap<>();"#,
    r#"    private volatile int workerCount = 0;"#,
    r#"    private final Object lock = new Object();"#,
    r#"    public record Span(int start, int end) {}"#,
    r#"    private static final Logger LOG = Logger.getLogger("indexer");"#,
];

const ITEMS_CSHARP: &[&str] = &[
    r#"    public const int MaxChunkSize = 4096;"#,
    r#"    private const int RetryLimit = 5;"#,
    r#"    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(30);"#,
    r#"    private readonly Dictionary<string, Span> _entries = new();"#,
    r#"    private readonly object _lock = new();"#,
    r#"    private int _workerCount;"#,
    r#"    public record Span(int Start, int End);"#,
    r#"    public int Count => _entries.Count;"#,
];

const BLOCKS_PYTHON: &[&str] = &[
    r#"def clamp(value: int, low: int, high: int) -> int:
    return max(low, min(value, high))"#,
    r#"def chunk_count(size: int) -> int:
    whole, rest = divmod(size, MAX_CHUNK_SIZE)
    return whole + bool(rest)"#,
    r#"def cache_path(key: str) -> Path:
    digest = hashlib.sha1(key.encode()).hexdigest()
    return CACHE_DIR / digest[:2] / digest"#,
    r#"def parse_chunk(data: bytes, offset: int = 0) -> bytes:
    end = min(offset + MAX_CHUNK_SIZE, len(data))
    return data[offset:end]"#,
    r#"@dataclass(frozen=True)
class Span:
    start: int
    end: int"#,
    r#"def checksum(data: bytes) -> int:
    acc = 0
    for byte in data:
        acc = ((acc << 5) | (acc >> 27)) ^ byte
    return acc & 0xFFFFFFFF"#,
    r#"def retry(op, limit: int = RETRY_LIMIT):
    for attempt in range(limit):
        try:
            return op()
        except OSError as exc:
            logger.warning("attempt %d failed: %s", attempt, exc)
    raise RuntimeError("retries exhausted")"#,
    r#"class Indexer:
    def __init__(self, capacity: int) -> None:
        self.entries: dict[str, Span] = {}
        self.capacity = capacity"#,
    r#"def lookup(cache: dict, key: str):
    hit = cache.get(key)
    if hit is None:
        return None
    return hit.copy()"#,
    r#"def is_empty(span: Span) -> bool:
    return span.start == span.end"#,
];

const BLOCKS_TYPESCRIPT: &[&str] = &[
    r#"function clamp(value: number, low: number, high: number) {
  return Math.max(low, Math.min(value, high));
}"#,
    r#"export function chunkCount(size: number): number {
  return Math.ceil(size / MAX_CHUNK_SIZE);
}"#,
    r#"function cachePath(key: string): string {
  const digest = createHash("sha1").update(key).digest("hex");
  return path.join(digest.slice(0, 2), digest);
}"#,
    r#"export function parseChunk(data: Buffer, offset = 0): Buffer {
  const end = Math.min(offset + MAX_CHUNK_SIZE, data.length);
  return data.subarray(offset, end);
}"#,
    r#"export interface Span {
  start: number;
  end: number;
}"#,
    r#"export function checksum(data: Uint8Array): number {
  let acc = 0;
  for (const byte of data) {
    acc = ((acc << 5) | (acc >>> 27)) ^ byte;
  }
  return acc >>> 0;
}"#,
    r#"async function retry<T>(op: () => Promise<T>): Promise<T> {
  for (let attempt = 0; attempt < RETRY_LIMIT; attempt++) {
    try {
      return await op();
    } catch (err) {
      console.warn(`attempt ${attempt} failed`, err);
    }
  }
  throw new Error("retries exhausted");
}"#,
    r#"export class Indexer {
  private entries = new Map<string, Span>();

  constructor(private capacity: number) {}
}"#,
    r#"const isEmpty = (span: Span): boolean => {
  return span.start === span.end;
};"#,
];

const BLOCKS_GO: &[&str] = &[
    r#"func clamp(value, low, high int) int {
    return max(low, min(value, high))
}"#,
    r#"func ChunkCount(size int) int {
    return (size + MaxChunkSize - 1) / MaxChunkSize
}"#,
    r#"func (ix *Indexer) Len() int {
    ix.mu.RLock()
    defer ix.mu.RUnlock()
    return len(ix.entries)
}"#,
    r#"func ParseChunk(data []byte, offset int) []byte {
    end := min(offset+MaxChunkSize, len(data))
    return data[offset:end]
}"#,
    r#"type Span struct {
    Start int
    End   int
}"#,
    r#"func Checksum(data []byte) uint32 {
    var acc uint32
    for _, b := range data {
        acc = bits.RotateLeft32(acc, 5) ^ uint32(b)
    }
    return acc
}"#,
    r#"func retry(op func() error) error {
    for attempt := 0; attempt < retryLimit; attempt++ {
        if err := op(); err == nil {
            return nil
        }
        log.Printf("attempt %d failed", attempt)
    }
    return ErrExhausted
}"#,
    r#"func NewIndexer(capacity int) *Indexer {
    return &Indexer{
        entries: make(map[string]Span, capacity),
    }
}"#,
    r#"func (s Span) Empty() bool {
    return s.Start == s.End
}"#,
];

const BLOCKS_RUST: &[&str] = &[
    r#"fn clamp(value: usize, low: usize, high: usize) -> usize {
    value.max(low).min(high)
}"#,
    r#"pub fn chunk_count(size: usize) -> usize {
    size.div_ceil(MAX_CHUNK_SIZE)
}"#,
    r#"impl Default for Config {
    fn default() -> Self {
        Self { workers: 4, timeout_ms: DEFAULT_TIMEOUT_MS }
    }
}"#,
    r#"fn parse_chunk(data: &[u8], offset: usize) -> Vec<u8> {
    let end = (offset + MAX_CHUNK_SIZE).min(data.len());
    data[offset..end].to_vec()
}"#,
    r#"impl Indexer {
    pub fn new(capacity: usize) -> Self {
        Self { entries: HashMap::with_capacity(capacity) }
    }
}"#,
    r#"pub fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, b| {
        acc.rotate_left(5) ^ u32::from(*b)
    })
}"#,
    r#"fn retry<F: Fn() -> Result<()>>(op: F) -> Result<()> {
    for attempt in 0..RETRY_LIMIT {
        if op().is_ok() {
            return Ok(());
        }
        log::warn!("attempt {} failed", attempt);
    }
    Err(Error::Exhausted)
}"#,
    r#"pub fn lookup(cache: &Cache, key: &str) -> Option<Vec<u8>> {
    match cache.get(key) {
        Some(hit) => Some(hit.clone()),
        None => None,
    }
}"#,
    r#"#[derive(Debug, Clone)]
pub struct Config {
    pub workers: usize,
    pub timeout_ms: u64,
}"#,
    r#"fn is_empty(span: &Span) -> bool {
    span.start == span.end
}"#,
];

const BLOCKS_JAVA: &[&str] = &[
    r#"    static int clamp(int value, int low, int high) {
        return Math.max(low, Math.min(value, high));
    }"#,
    r#"    static int chunkCount(int size) {
        return (size + MAX_CHUNK_SIZE - 1) / MAX_CHUNK_SIZE;
    }"#,
    r#"    void clear() {
        synchronized (lock) {
            entries.clear();
        }
    }"#,
    r#"    static byte[] parseChunk(byte[] data, int offset) {
        int end = Math.min(offset + MAX_CHUNK_SIZE, data.length);
        return Arrays.copyOfRange(data, offset, end);
    }"#,
    r#"    static int checksum(byte[] data) {
        int acc = 0;
        for (byte b : data) {
            acc = Integer.rotateLeft(acc, 5) ^ (b & 0xFF);
        }
        return acc;
    }"#,
    r#"    <T> T retry(Supplier<T> op) {
        for (int attempt = 0; attempt < RETRY_LIMIT; attempt++) {
            try {
                return op.get();
            } catch (RuntimeException e) {
                LOG.warning("attempt " + attempt + " failed");
            }
        }
        throw new IllegalStateException("retries exhausted");
    }"#,
    r#"    Span lookup(String key) {
        synchronized (lock) {
            return entries.get(key);
        }
    }"#,
    r#"    boolean isEmpty(Span span) {
        return span.start() == span.end();
    }"#,
    r#"    public int size() {
        return entries.size();
    }"#,
];

const BLOCKS_CSHARP: &[&str] = &[
    r#"    private static int Clamp(int value, int low, int high)
    {
        return Math.Max(low, Math.Min(value, high));
    }"#,
    r#"    public static int ChunkCount(int size) => (size + MaxChunkSize - 1) / MaxChunkSize;"#,
    r#"    public void Clear()
    {
        lock (_lock) { _entries.Clear(); }
    }"#,
    r#"    public static byte[] ParseChunk(byte[] data, int offset)
    {
        var end = Math.Min(offset + MaxChunkSize, data.Length);
        return data[offset..end];
    }"#,
    r#"    public static uint Checksum(ReadOnlySpan<byte> data)
    {
        uint acc = 0;
        foreach (var b in data)
        {
            acc = BitOperations.RotateLeft(acc, 5) ^ b;
        }
        return acc;
    }"#,
    r#"    private T Retry<T>(Func<T> op)
    {
        for (var attempt = 0; attempt < RetryLimit; attempt++)
        {
            try { return op(); }
            catch (IOException) { _workerCount++; }
        }
        throw new InvalidOperationException("retries exhausted");
    }"#,
    r#"    public Span? Lookup(string key)
    {
        lock (_lock)
        {
            return _entries.GetValueOrDefault(key);
        }
    }"#,
    r#"    private static bool IsEmpty(Span span)
    {
        return span.Start == span.End;
    }"#,
];
