#!/usr/bin/env python3

from datetime import date
import sys


today: date = date.today()
title_raw: list[str] = sys.argv[1:]

if not title_raw:
    raise RuntimeError("Title cannot be empty")

title_spaced: str = " ".join(title_raw)
title_dashed: str = "-".join(title_raw)
today_dashed: str = today.strftime("%Y-%m-%d")

# only open the file for writing if it does not exist,
# with "x" mode
with open(f"{today_dashed}-{title_dashed}.md", "x") as f:
    f.write("---\n")
    f.write("layout: post\n")
    f.write(f"title: {title_spaced}\n")
    f.write(f"created: {today_dashed}\n")
    f.write("---\n\n\n")
