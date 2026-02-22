#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.6"
# dependencies = []
# ///

from datetime import date
import sys


today: date = date.today()
title_raw: list[str] = sys.argv[1:]

if not title_raw:
    raise RuntimeError("Title cannot be empty")

title_spaced: str = " ".join(title_raw)
title_dashed: str = "-".join(title_raw)
today_dashed: str = today.strftime("%Y-%m-%d")

post = f"""---
layout: post
title: {title_spaced}
created: {today_dashed}
---

"""

# only open the file for writing if it does not exist,
# with "x" mode
with open(f"{today_dashed}-{title_dashed}.md", "x", encoding="utf-8") as f:
    f.write(post)
