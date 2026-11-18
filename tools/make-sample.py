#!/usr/bin/env python3
"""Writes web/samples/demo.json: a synthetic wiki-style history that shows
each pattern the visualization exists to reveal. It is the page's default
document and the subject of the e2e goldens.

The history is generated from a fixed seed, so the output is the same on
every run; it is committed, and this script is how it was made.

    growth        several authors adding paragraphs over time
    anonymous     two authors with no account (drawn in greys)
    vandalism     the page blanked twice and restored a minute later
    edit war      two authors swapping a paragraph back and forth
    move          a paragraph moved from the end to the start
    bursts        edits seconds apart between gaps of days (date spacing)
"""
import json
import random
from pathlib import Path

rng = random.Random(2004)
WORDS = """history flow visual document author edit page text version wiki
community revision sentence change time pattern growth conflict cooperation
vandalism repair stable line colour ribbon length article people together
shared record trace survive rewrite argue restore discuss agree knowledge
public open simple rapid careful small large early late first lasting""".split()


def sentence():
    words = [rng.choice(WORDS) for _ in range(rng.randint(5, 14))]
    return " ".join(words).capitalize() + "."


def paragraph():
    return " ".join(sentence() for _ in range(rng.randint(2, 5)))


versions = []
clock = 1_070_000_000  # late 2003
page = []


def save(author, comment, gap, anonymous=False, text=None):
    global clock
    clock += gap
    versions.append({
        "id": str(1000 + len(versions)),
        "author": author,
        "anonymous": anonymous,
        "timestamp": clock,
        "comment": comment,
        "text": "\n\n".join(page) if text is None else text,
    })


DAY = 86_400
NAMED = ["Ada", "Brooke", "Chidi", "Dmitri", "Esther"]

page.append(paragraph())
save("Ada", "Start the page", 0)
for step in range(46):
    author = rng.choice(NAMED)
    roll = rng.random()
    gap = rng.choice([40, 90, 600, DAY, 3 * DAY, 9 * DAY])

    if step == 14 or step == 31:
        # Vandalism: blanked by an anonymous author, restored by a named one.
        save("203.0.113.7", "", gap, anonymous=True, text="This page is rubbish.")
        save(rng.choice(NAMED), "Revert vandalism", 70)
        continue
    if step == 22:
        # Edit war: Brooke and Dmitri alternate over one paragraph.
        target = len(page) // 2
        theirs, ours = page[target], paragraph()
        for round_ in range(3):
            page[target] = ours
            save("Dmitri", "This is the accurate wording", 300)
            page[target] = theirs
            save("Brooke", "Restore the agreed text", 240)
        continue
    if step == 37:
        page.insert(0, page.pop())
        save(author, "Move the summary to the top", gap)
        continue

    if roll < 0.5 or len(page) < 3:
        page.insert(rng.randint(0, len(page)), paragraph())
        comment = "Add a paragraph"
    elif roll < 0.72:
        at = rng.randrange(len(page))
        page[at] = page[at] + " " + sentence()
        comment = "Expand"
    elif roll < 0.88:
        at = rng.randrange(len(page))
        sentences = page[at].split(". ")
        sentences[rng.randrange(len(sentences))] = sentence().rstrip(".")
        page[at] = ". ".join(sentences)
        comment = "Reword a sentence"
    elif roll < 0.95:
        page.append(sentence())
        save("198.51.100.23", "", gap, anonymous=True)
        continue
    else:
        page.pop(rng.randrange(len(page)))
        comment = "Remove a paragraph"
    save(author, comment, gap)

bundle = {
    "format": "historyflow-bundle/1",
    "title": "Demo: a synthetic wiki page",
    "mode": "prose",
    "versions": versions,
}
out = Path(__file__).resolve().parent.parent / "web" / "samples" / "demo.json"
out.write_text(json.dumps(bundle, indent=1) + "\n")
print(f"{out}: {len(versions)} versions")
