from __future__ import annotations

from random import Random

from fcc.book import Paragraph, Pos
from fcc.errors import FccError
from fcc.masks.base import Line, error_lines, fit_lines, plain_line, progress_bar_label
from fcc.text import wrap

LANGS = ("python", "typescript", "go", "rust", "java", "csharp")

_HEADERS = {
    "python": ["import os", "import sys", "from typing import Optional"],
    "typescript": ['import { readFileSync } from "fs";', 'import path from "path";'],
    "go": ["package main", 'import "fmt"'],
    "rust": ["use std::collections::HashMap;", "use std::io;"],
    "java": ["package com.example.app;", "import java.util.List;"],
    "csharp": ["using System;", "using System.Linq;"],
}

_NOISE = {
    "python": [
        "def parse_chunk(data, offset=0):",
        "    return data[offset:]",
        "    result = self._cache.get(key)",
        "class Indexer(object):",
        "    items = [x for x in items if x]",
        "    if node is None:",
    ],
    "typescript": [
        "function parseChunk(data: string): string {",
        "  return data.slice(offset);",
        "  const result = cache.get(key);",
        "export class Indexer {",
        "  let items = items.filter(Boolean);",
    ],
    "go": [
        "func parseChunk(data []byte) []byte {",
        "\treturn data[offset:]",
        "\tresult := cache[key]",
        "type Indexer struct {",
        "\tif err != nil {",
    ],
    "rust": [
        "fn parse_chunk(data: &[u8]) -> Vec<u8> {",
        "    return data[offset..].to_vec();",
        "    let result = cache.get(&key);",
        "pub struct Indexer {",
        "    let items: Vec<_> = items.iter().collect();",
    ],
    "java": [
        "public String parseChunk(String data) {",
        "    return data.substring(offset);",
        "    Object result = cache.get(key);",
        "public class Indexer {",
        "    List<String> items = new ArrayList<>();",
    ],
    "csharp": [
        "public string ParseChunk(string data) {",
        "    return data.Substring(offset);",
        "    var result = cache[key];",
        "public class Indexer {",
        "    var items = items.ToList();",
    ],
}

_NUM_WIDTH = 5  # "%4d " gutter


class CodeMask:
    name = "code"

    def __init__(self, lang: str) -> None:
        if lang not in LANGS:
            raise ValueError(f"unknown language: {lang!r}")
        self.lang = lang
        # number for the next stand-alone noise line: continues the last rendered
        # block so filler between paragraphs carries a gutter like every other line
        self._next_no = 1

    def header(self, rng: Random, width: int) -> list[Line]:
        return fit_lines([plain_line(t, "dim green") for t in _HEADERS[self.lang]], width)

    def _noise_text(self, rng: Random) -> str:
        return rng.choice(_NOISE[self.lang])

    def noise(self, rng: Random, width: int) -> Line:
        gutter = f"{self._next_no:>4} "
        self._next_no += 1
        text = self._noise_text(rng)[: max(0, width - len(gutter))]
        return Line(((gutter[: max(1, width)], "dim"), (text, "green")), False)

    def _frame(self) -> tuple[str, list[str], list[str]]:
        """(prefix for each text line, opening lines, closing lines)."""
        if self.lang == "python":
            return "    ", ['"""'], ['"""']
        if self.lang == "rust":
            return "/// ", [], []
        return " * ", ["/**"], [" */"]

    def render(
        self,
        para: Paragraph,
        pos: Pos,
        width: int,
        rng: Random,
        line_no_start: int = 1,
    ) -> list[Line]:
        prefix, opening, closing = self._frame()
        body_width = max(1, width - _NUM_WIDTH - len(prefix))
        n = line_no_start
        out: list[Line] = []

        def emit(text: str, style: str, book: bool = False) -> None:
            nonlocal n
            segs = ((f"{n:>4} ", "dim"), (text, style))
            out.append(Line(segs, book, pos if book else None))
            n += 1

        avail = max(1, width - _NUM_WIDTH)
        emit(self._noise_text(rng)[:avail], "green")
        for t in opening:
            emit(t, "dim green")
        for text in wrap(para.text, body_width):
            segs = (
                (f"{n:>4} ", "dim"),
                (prefix, "dim green"),
                (text, "book dim green"),
            )
            out.append(Line(segs, True, pos))
            n += 1
        for t in closing:
            emit(t, "dim green")
        emit(self._noise_text(rng)[:avail], "green")
        self._next_no = n
        return out

    def error(self, err: FccError, width: int) -> list[Line]:
        return error_lines(err, width)

    def progress_label(self, done: int, total: int, width: int) -> str:
        return progress_bar_label(done, total)
