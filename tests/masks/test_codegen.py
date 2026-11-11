"""Shuffled no-repeat pools for generated filler."""
import ast

from fcc.masks.codegen import (
    COMMANDS,
    FILES,
    SIGNATURES,
    STATEMENTS,
    TASKS,
    Deck,
)


def test_deck_never_repeats_within_its_size_and_is_stable():
    deck = Deck([f"item{i}" for i in range(50)], seed="t")
    first = [deck[i] for i in range(50)]
    assert len(set(first)) == 50
    assert [deck[i] for i in range(50, 100)] == first  # then the same order again
    assert Deck([f"item{i}" for i in range(50)], seed="t")[7] == first[7]


def test_deck_order_is_shuffled_not_the_input_order():
    items = [f"item{i}" for i in range(50)]
    assert [Deck(items, seed="t")[i] for i in range(50)] != items


def test_deck_dedupes_its_items():
    assert len(Deck(["a", "b", "a"], seed="x")) == 2


def test_pools_are_large():
    assert len(STATEMENTS) >= 2000
    assert len(SIGNATURES) >= 300
    assert len(FILES) >= 200
    assert len(COMMANDS) >= 200
    assert len(TASKS) >= 100


def test_generated_python_parses():
    for i in range(0, len(STATEMENTS), 7):
        line = STATEMENTS[i].strip()
        body = "\n        pass" if line.endswith(":") else ""
        ast.parse(f"async def _():\n    {line}{body}")  # inside a function: await/yield/return are fine
    for i in range(0, len(SIGNATURES), 5):
        ast.parse(SIGNATURES[i].strip() + "\n    pass")


def test_no_forbidden_words_in_pools():
    forbidden = ("book", "reader", "okuyucu")
    for deck in (STATEMENTS, SIGNATURES, FILES, COMMANDS, TASKS):
        for i in range(len(deck)):
            assert not any(w in deck[i].lower() for w in forbidden), deck[i]
