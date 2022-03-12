#!/usr/bin/env python3

from datetime import date
import sys

def main():
    today = date.today()
    title_raw = sys.argv[1:]
    title_spaced = " ".join(title_raw)
    title_dashed = "-".join(title_raw)
    today_dashed = today.strftime("%Y-%m-%d")

    # only open the file for writing if it does not exist,
    # with "x" mode
    with open(f"{today_dashed}-{title_dashed}.md", "x") as f:
        f.write("---\n")
        f.write("layout: post\n")
        f.write(f"title: {title_spaced}\n")
        f.write(f"created: {today_dashed}\n")
        f.write("---\n\n\n")


if __name__ == "__main__":
    sys.exit(main())
