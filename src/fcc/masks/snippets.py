"""Filler source code per language for the code mask.

HEADERS open the file, ITEMS are one-line declarations that sit under a doc
comment, BLOCKS are complete multi-line definitions.
"""

from __future__ import annotations

HEADERS = {
    "python": """\
from __future__ import annotations

import logging
from dataclasses import dataclass
from pathlib import Path""",
    "typescript": """\
import { createHash } from "crypto";
import path from "path";

import type { Config, Span } from "./types";""",
    "go": """\
package indexer

import (
    "errors"
    "sync"
)""",
    "rust": """\
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::{Error, Result};""",
    "java": """\
package com.example.indexer;

import java.util.HashMap;
import java.util.Map;

public final class Indexer {""",
    "csharp": """\
using System;
using System.Collections.Generic;

namespace Example.Indexing;

public sealed class Indexer
{""",
}

ITEMS = {
    "python": [
        "MAX_CHUNK_SIZE = 4096",
        "DEFAULT_TIMEOUT = 30.0",
        "RETRY_LIMIT = 5",
        "logger = logging.getLogger(__name__)",
        'CACHE_DIR = Path.home() / ".cache" / "indexer"',
        "_registry: dict[str, type] = {}",
        'ENCODINGS = ("utf-8", "cp1254", "latin-1")',
        "Handler = Callable[[bytes], bytes]",
    ],
    "typescript": [
        "export const MAX_CHUNK_SIZE = 4096;",
        "export const DEFAULT_TIMEOUT_MS = 30_000;",
        "const RETRY_LIMIT = 5;",
        "export type Handler = (data: Buffer) => Buffer;",
        "const registry = new Map<string, Handler>();",
        'export const ENCODINGS = ["utf-8", "latin1"] as const;',
        "let workerCount = 0;",
        "export type Cache = Map<string, Uint8Array>;",
    ],
    "go": [
        "const MaxChunkSize = 4096",
        "const DefaultTimeout = 30 * time.Second",
        "const retryLimit = 5",
        'var ErrExhausted = errors.New("retries exhausted")',
        "var workerCount atomic.Int64",
        "type Handler func(data []byte) []byte",
        "type Cache map[string][]byte",
        "var registry = map[string]Handler{}",
    ],
    "rust": [
        "pub const MAX_CHUNK_SIZE: usize = 4096;",
        "pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;",
        "const RETRY_LIMIT: u32 = 5;",
        "pub type Shared<T> = Arc<RwLock<T>>;",
        "pub struct Token(pub u64);",
        "static WORKER_COUNT: AtomicUsize = AtomicUsize::new(0);",
        "pub struct Span { pub start: usize, pub end: usize }",
        "type Cache = HashMap<String, Vec<u8>>;",
    ],
    "java": [
        "    public static final int MAX_CHUNK_SIZE = 4096;",
        "    private static final long DEFAULT_TIMEOUT_MS = 30_000L;",
        "    private static final int RETRY_LIMIT = 5;",
        "    private final Map<String, Span> entries = new HashMap<>();",
        "    private volatile int workerCount = 0;",
        "    private final Object lock = new Object();",
        "    public record Span(int start, int end) {}",
        '    private static final Logger LOG = Logger.getLogger("indexer");',
    ],
    "csharp": [
        "    public const int MaxChunkSize = 4096;",
        "    private const int RetryLimit = 5;",
        "    private static readonly TimeSpan Timeout = TimeSpan.FromSeconds(30);",
        "    private readonly Dictionary<string, Span> _entries = new();",
        "    private readonly object _lock = new();",
        "    private int _workerCount;",
        "    public record Span(int Start, int End);",
        "    public int Count => _entries.Count;",
    ],
}

BLOCKS = {
    "python": [
        """\
def clamp(value: int, low: int, high: int) -> int:
    return max(low, min(value, high))""",
        """\
def chunk_count(size: int) -> int:
    whole, rest = divmod(size, MAX_CHUNK_SIZE)
    return whole + bool(rest)""",
        """\
def cache_path(key: str) -> Path:
    digest = hashlib.sha1(key.encode()).hexdigest()
    return CACHE_DIR / digest[:2] / digest""",
        """\
def parse_chunk(data: bytes, offset: int = 0) -> bytes:
    end = min(offset + MAX_CHUNK_SIZE, len(data))
    return data[offset:end]""",
        """\
@dataclass(frozen=True)
class Span:
    start: int
    end: int""",
        """\
def checksum(data: bytes) -> int:
    acc = 0
    for byte in data:
        acc = ((acc << 5) | (acc >> 27)) ^ byte
    return acc & 0xFFFFFFFF""",
        """\
def retry(op, limit: int = RETRY_LIMIT):
    for attempt in range(limit):
        try:
            return op()
        except OSError as exc:
            logger.warning("attempt %d failed: %s", attempt, exc)
    raise RuntimeError("retries exhausted")""",
        """\
class Indexer:
    def __init__(self, capacity: int) -> None:
        self.entries: dict[str, Span] = {}
        self.capacity = capacity""",
        """\
def lookup(cache: dict, key: str):
    hit = cache.get(key)
    if hit is None:
        return None
    return hit.copy()""",
        """\
def is_empty(span: Span) -> bool:
    return span.start == span.end""",
    ],
    "typescript": [
        """\
function clamp(value: number, low: number, high: number) {
  return Math.max(low, Math.min(value, high));
}""",
        """\
export function chunkCount(size: number): number {
  return Math.ceil(size / MAX_CHUNK_SIZE);
}""",
        """\
function cachePath(key: string): string {
  const digest = createHash("sha1").update(key).digest("hex");
  return path.join(digest.slice(0, 2), digest);
}""",
        """\
export function parseChunk(data: Buffer, offset = 0): Buffer {
  const end = Math.min(offset + MAX_CHUNK_SIZE, data.length);
  return data.subarray(offset, end);
}""",
        """\
export interface Span {
  start: number;
  end: number;
}""",
        """\
export function checksum(data: Uint8Array): number {
  let acc = 0;
  for (const byte of data) {
    acc = ((acc << 5) | (acc >>> 27)) ^ byte;
  }
  return acc >>> 0;
}""",
        """\
async function retry<T>(op: () => Promise<T>): Promise<T> {
  for (let attempt = 0; attempt < RETRY_LIMIT; attempt++) {
    try {
      return await op();
    } catch (err) {
      console.warn(`attempt ${attempt} failed`, err);
    }
  }
  throw new Error("retries exhausted");
}""",
        """\
export class Indexer {
  private entries = new Map<string, Span>();

  constructor(private capacity: number) {}
}""",
        """\
const isEmpty = (span: Span): boolean => {
  return span.start === span.end;
};""",
    ],
    "go": [
        """\
func clamp(value, low, high int) int {
    return max(low, min(value, high))
}""",
        """\
func ChunkCount(size int) int {
    return (size + MaxChunkSize - 1) / MaxChunkSize
}""",
        """\
func (ix *Indexer) Len() int {
    ix.mu.RLock()
    defer ix.mu.RUnlock()
    return len(ix.entries)
}""",
        """\
func ParseChunk(data []byte, offset int) []byte {
    end := min(offset+MaxChunkSize, len(data))
    return data[offset:end]
}""",
        """\
type Span struct {
    Start int
    End   int
}""",
        """\
func Checksum(data []byte) uint32 {
    var acc uint32
    for _, b := range data {
        acc = bits.RotateLeft32(acc, 5) ^ uint32(b)
    }
    return acc
}""",
        """\
func retry(op func() error) error {
    for attempt := 0; attempt < retryLimit; attempt++ {
        if err := op(); err == nil {
            return nil
        }
        log.Printf("attempt %d failed", attempt)
    }
    return ErrExhausted
}""",
        """\
func NewIndexer(capacity int) *Indexer {
    return &Indexer{
        entries: make(map[string]Span, capacity),
    }
}""",
        """\
func (s Span) Empty() bool {
    return s.Start == s.End
}""",
    ],
    "rust": [
        """\
fn clamp(value: usize, low: usize, high: usize) -> usize {
    value.max(low).min(high)
}""",
        """\
pub fn chunk_count(size: usize) -> usize {
    size.div_ceil(MAX_CHUNK_SIZE)
}""",
        """\
impl Default for Config {
    fn default() -> Self {
        Self { workers: 4, timeout_ms: DEFAULT_TIMEOUT_MS }
    }
}""",
        """\
fn parse_chunk(data: &[u8], offset: usize) -> Vec<u8> {
    let end = (offset + MAX_CHUNK_SIZE).min(data.len());
    data[offset..end].to_vec()
}""",
        """\
impl Indexer {
    pub fn new(capacity: usize) -> Self {
        Self { entries: HashMap::with_capacity(capacity) }
    }
}""",
        """\
pub fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, b| {
        acc.rotate_left(5) ^ u32::from(*b)
    })
}""",
        """\
fn retry<F: Fn() -> Result<()>>(op: F) -> Result<()> {
    for attempt in 0..RETRY_LIMIT {
        if op().is_ok() {
            return Ok(());
        }
        log::warn!("attempt {} failed", attempt);
    }
    Err(Error::Exhausted)
}""",
        """\
pub fn lookup(cache: &Cache, key: &str) -> Option<Vec<u8>> {
    match cache.get(key) {
        Some(hit) => Some(hit.clone()),
        None => None,
    }
}""",
        """\
#[derive(Debug, Clone)]
pub struct Config {
    pub workers: usize,
    pub timeout_ms: u64,
}""",
        """\
fn is_empty(span: &Span) -> bool {
    span.start == span.end
}""",
    ],
    "java": [
        """\
    static int clamp(int value, int low, int high) {
        return Math.max(low, Math.min(value, high));
    }""",
        """\
    static int chunkCount(int size) {
        return (size + MAX_CHUNK_SIZE - 1) / MAX_CHUNK_SIZE;
    }""",
        """\
    void clear() {
        synchronized (lock) {
            entries.clear();
        }
    }""",
        """\
    static byte[] parseChunk(byte[] data, int offset) {
        int end = Math.min(offset + MAX_CHUNK_SIZE, data.length);
        return Arrays.copyOfRange(data, offset, end);
    }""",
        """\
    static int checksum(byte[] data) {
        int acc = 0;
        for (byte b : data) {
            acc = Integer.rotateLeft(acc, 5) ^ (b & 0xFF);
        }
        return acc;
    }""",
        """\
    <T> T retry(Supplier<T> op) {
        for (int attempt = 0; attempt < RETRY_LIMIT; attempt++) {
            try {
                return op.get();
            } catch (RuntimeException e) {
                LOG.warning("attempt " + attempt + " failed");
            }
        }
        throw new IllegalStateException("retries exhausted");
    }""",
        """\
    Span lookup(String key) {
        synchronized (lock) {
            return entries.get(key);
        }
    }""",
        """\
    boolean isEmpty(Span span) {
        return span.start() == span.end();
    }""",
        """\
    public int size() {
        return entries.size();
    }""",
    ],
    "csharp": [
        """\
    private static int Clamp(int value, int low, int high)
    {
        return Math.Max(low, Math.Min(value, high));
    }""",
        """\
    public static int ChunkCount(int size) => (size + MaxChunkSize - 1) / MaxChunkSize;""",
        """\
    public void Clear()
    {
        lock (_lock) { _entries.Clear(); }
    }""",
        """\
    public static byte[] ParseChunk(byte[] data, int offset)
    {
        var end = Math.Min(offset + MaxChunkSize, data.Length);
        return data[offset..end];
    }""",
        """\
    public static uint Checksum(ReadOnlySpan<byte> data)
    {
        uint acc = 0;
        foreach (var b in data)
        {
            acc = BitOperations.RotateLeft(acc, 5) ^ b;
        }
        return acc;
    }""",
        """\
    private T Retry<T>(Func<T> op)
    {
        for (var attempt = 0; attempt < RetryLimit; attempt++)
        {
            try { return op(); }
            catch (IOException) { _workerCount++; }
        }
        throw new InvalidOperationException("retries exhausted");
    }""",
        """\
    public Span? Lookup(string key)
    {
        lock (_lock)
        {
            return _entries.GetValueOrDefault(key);
        }
    }""",
        """\
    private static bool IsEmpty(Span span)
    {
        return span.Start == span.End;
    }""",
    ],
}
