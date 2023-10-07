#!/usr/bin/env elixir

Mix.install(tzdata: "~> 1.1")
Calendar.put_time_zone_database(Tzdata.TimeZoneDatabase)

today = DateTime.now!("America/Chicago")
title_raw = System.argv()

if Enum.empty?(title_raw) do
  raise "Title cannot be empty"
end

title_spaced = Enum.join(title_raw, " ")
title_dashed = Enum.join(title_raw, "-")
today_dashed = Calendar.strftime(today, "%Y-%m-%d")

post = [
  "---",
  "layout: post",
  "title: #{title_spaced}",
  "created: #{today_dashed}",
  "---",
  "\n\n"
]

post = Enum.intersperse(post, "\n")

File.write!("#{today_dashed}-#{title_dashed}.md", post, [:exclusive])
