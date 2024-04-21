use anyhow::{Context, Result};
use chrono::Utc;
use glob::glob;
use maud::{html, Markup, PreEscaped, DOCTYPE};
use pulldown_cmark::{html, Parser};
use regex::Regex;
use rss::{ChannelBuilder, ItemBuilder};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct Post<'a> {
    title: &'a str,
    created_on: chrono::NaiveDate,
    body: Markup,
}

struct Page<'a> {
    title: &'a str,
    body: Markup,
}

fn md_to_html(markdown_str: &str) -> Markup {
    let parser = Parser::new(markdown_str);
    let mut html_buf = String::new();
    html::push_html(&mut html_buf, parser);
    maud::PreEscaped(html_buf)
}

fn parse_post(s: &str) -> Result<Post> {
    static POST_REGEX: std::sync::OnceLock<Regex> = OnceLock::new();

    POST_REGEX.get_or_init(|| {
        Regex::new(
            r"---
layout: post
title: (?P<title>.+)
created: (?P<created_on>\d{4}-\d{2}-\d{2})
---
(?s)
(?P<body>.*)",
        )
        .unwrap()
    });

    let captures = POST_REGEX.get().unwrap().captures(s).unwrap();

    Ok(Post {
        title: captures.name("title").unwrap().as_str(),
        created_on: chrono::NaiveDate::parse_from_str(&captures["created_on"], "%Y-%m-%d")?,
        body: md_to_html(&captures["body"]),
    })
}

fn parse_page(s: &str) -> Result<Page> {
    static PAGE_REGEX: OnceLock<Regex> = OnceLock::new();

    PAGE_REGEX.get_or_init(|| {
        Regex::new(
            r"---
title: (?P<title>.+)
---
(?s)
(?P<body>.+)",
        )
        .unwrap()
    });

    let captures = PAGE_REGEX.get().unwrap().captures(s).unwrap();

    Ok(Page {
        title: captures.name("title").unwrap().as_str(),
        body: md_to_html(&captures["body"]),
    })
}

fn get_markdown_files(path: &Path) -> Result<glob::Paths, glob::PatternError> {
    let mdpath = path.join("**/*.md");
    let mdpathstr = mdpath
        .to_str()
        .expect("must be able to convert path to str");
    glob(mdpathstr)
}

macro_rules! layout {
    ($title:expr, $content:expr) => {
        html! {
            (DOCTYPE)
            head {
                meta charset="utf-8";
                meta content="IE=edge,chrome=1" http-equiv="X-UA-Compatible";
                title { ($title) }
                meta content="width=device-width" name="viewport";
                link rel="icon" href="favicon-min.png" type="image.png";
            }
            body {
                div.container {
                    div.site {
                        div.header {
                            h1.title {
                                a href="index.html" {
                                    "Clark Kampfe"
                                }
                            }

                            a.extra href="about.html" {
                                "about"
                            }
                            " "
                            a.extra href="resume.html" {
                                "resumé"
                            }
                        }
                        ($content)
                        div.footer {
                            div.contact {
                                p {
                                    a href="https://github.com/ckampfe/" {
                                        "github"
                                    }
                                    " "
                                    a href="https://twitter.com/clarkkampfe" {
                                        "twitter"
                                    }
                                    " "
                                    a href="/feed" {
                                        "rss"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };
}

fn page(title: &str, content: &Markup) -> Markup {
    layout!(
        title,
        html! {
            div {
                h1 { (title) }
                div.page { (content) }
            }
        }
    )
}

fn post(title: &str, created: &str, content: &Markup) -> Markup {
    layout!(
        title,
        html! {
            div {
                h2 { (PreEscaped(title)) }
                p.meta { (created) }
                div.post { (content) }
            }
        }
    )
}

fn index_link(filename: &str, title: &str, created_at: &str) -> Markup {
    html! {
        li {
            a href=(filename) {
                (PreEscaped(title))
            }
            " "
            span {
                (created_at)
            }
        }
    }
}

fn index(post_links: &[Markup]) -> Markup {
    layout!(
        "Clark Kampfe - zeroclarkthirty.com",
        html! {
            div #home {
                ul.posts {
                    @for post_link in post_links {
                        (post_link)
                    }
                }
            }
        }
    )
}

fn rss_feed() -> rss::Channel {
    ChannelBuilder::default()
        .title("Clark Kampfe - zeroclarkthirty.com")
        .link("https://zeroclarkthirty.com")
        .description("zeroclarkthirty.com")
        .build()
}

fn rss_item(post: Post, link: &str) -> rss::Item {
    let t = chrono::NaiveTime::from_hms_milli_opt(0, 0, 0, 0).unwrap();
    let dt = chrono::DateTime::<Utc>::from_naive_utc_and_offset(
        post.created_on.and_time(t),
        chrono::Utc,
    )
    .to_rfc2822();
    ItemBuilder::default()
        .title(post.title.to_string())
        .link(link.to_owned())
        .content(post.body.0)
        .pub_date(dt)
        .build()
}

fn main() -> Result<()> {
    let mut conn = rusqlite::Connection::open("zct.db")?;

    let cwd = std::env::current_dir().context("Could not get current working directory")?;
    let build_dir = cwd.join("build");
    std::fs::create_dir_all(&build_dir).context("Could not create build dir")?;

    let post_paths = get_markdown_files(&cwd.join("posts"))
        .with_context(|| "Could not get markdown files for posts")?
        .collect::<Vec<_>>();

    let mut feed = rss_feed();
    let mut rss_items = Vec::with_capacity(post_paths.len());
    let mut index_links = Vec::with_capacity(post_paths.len());
    let mut paths_and_content: Vec<(PathBuf, String)> = Vec::with_capacity(post_paths.len());

    for post_path in post_paths {
        let post_path = post_path?;
        let content = std::fs::read_to_string(&post_path)
            .with_context(|| format!("Could not read post {:?}", post_path))?;
        paths_and_content.push((post_path, content));
    }

    let mut paths_and_posts = Vec::with_capacity(paths_and_content.len());

    for (post_path, content) in &paths_and_content {
        let post = parse_post(content)?;
        paths_and_posts.push((post_path, post))
    }

    paths_and_posts.sort_unstable_by(|a, b| b.1.created_on.cmp(&a.1.created_on));

    let mut post_output_path = PathBuf::new();

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    tx.execute(
        "create table if not exists posts (id integer primary key, title text, body text, created_on text)",
        [],
    )?;
    tx.execute(
        "create unique index if not exists posts_title_created_on on posts (title, created_on)",
        [],
    )?;

    tx.execute(
        "create virtual table if not exists posts_search using fts5(title, body, created_on)",
        [],
    )?;

    tx.execute("create trigger if not exists posts_fts5
    after insert on posts
    for each row
    begin
        insert into posts_search (title, body, created_on) values (new.title, new.body, new.created_on);
    end;
    ", [])?;

    for (post_path, post) in paths_and_posts {
        tx.execute(
            "insert into posts (title, body, created_on) values (?, ?, ?) on conflict (title, created_on) do update set body = excluded.body",
            rusqlite::params![
                &post.title,
                html2text::from_read(post.body.clone().into_string().as_bytes(), 150),
                // &post.body.clone().into_string(),
                &post.created_on.to_string()
            ],
        )?;

        let post_created_on = &post.created_on.format("%Y-%m-%d");

        let post_layout_html = crate::post(post.title, &post_created_on.to_string(), &post.body);

        let filename = post_path
            .file_name()
            .expect("Could not make post path into str");

        post_output_path.clear();
        post_output_path.push(&build_dir);
        post_output_path.push(filename);
        post_output_path.set_extension("html");

        let mut post_output = std::fs::File::create(&post_output_path).with_context(|| {
            format!("Could not create post output path: {:?}", &post_output_path)
        })?;

        post_output
            .write_all(post_layout_html.into_string().as_bytes())
            .with_context(|| {
                format!(
                    "Could not write post output html to {:?}",
                    &post_output_path
                )
            })?;

        let mut index_link_post_path = PathBuf::new();

        index_link_post_path.push(filename);
        index_link_post_path.set_extension("html");

        let index_link_post_str = index_link_post_path
            .to_str()
            .expect("Could not create filename from osstr");

        let index_link_html = index_link(
            index_link_post_str,
            post.title,
            &post_created_on.to_string(),
        );

        index_links.push(index_link_html);

        let mut post_link = PathBuf::new();
        post_link.push("https://zeroclarkthirty.com");
        post_link.push(filename);
        post_link.set_extension("html");
        let post_link_str = post_link.to_str().expect("Could not convert link to str");
        let post_rss_item = rss_item(post, post_link_str);
        rss_items.push(post_rss_item);
    }

    let index_layout_html = index(&index_links);

    let mut index_output_path = PathBuf::new();
    index_output_path.push(&build_dir);
    index_output_path.push("index");
    index_output_path.set_extension("html");
    let mut index_output = std::fs::File::create(index_output_path)?;
    index_output.write_all(index_layout_html.into_string().as_bytes())?;

    feed.set_items(rss_items);
    let mut rss_feed_path = PathBuf::new();
    rss_feed_path.push(&build_dir);
    rss_feed_path.push("feed");
    let feed_file = std::fs::File::create(rss_feed_path)?;

    feed.write_to(feed_file)?;

    let page_paths = get_markdown_files(&cwd.join("pages"))?;

    tx.execute(
        "create table if not exists pages (id integer primary key, title text, body text)",
        [],
    )?;

    tx.execute(
        "create unique index if not exists pages_title on pages (title)",
        [],
    )?;

    for page_path in page_paths {
        let pp = page_path?;
        let contents =
            std::fs::read_to_string(&pp).with_context(|| format!("Could not read {:?}", pp))?;
        let page = parse_page(&contents)?;

        let page_layout_html = crate::page(page.title, &page.body);

        tx.execute(
            "insert into pages (title, body) values (?, ?) on conflict (title) do update set body = excluded.body",
            rusqlite::params![page.title, page.body.clone().into_string()],
        )?;

        let filename = pp.file_name().expect("Could not make page path into str");
        let mut page_output_path = PathBuf::new();
        page_output_path.push(&build_dir);
        page_output_path.push(filename);
        page_output_path.set_extension("html");
        let mut page_output = std::fs::File::create(&page_output_path)
            .with_context(|| format!("Could not create {:?}", page_output_path))?;
        page_output
            .write_all(page_layout_html.into_string().as_bytes())
            .with_context(|| format!("Could not write page to {:?}", page_output_path))?;
    }

    tx.commit()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn recognizes_a_post() {
        let post_text = r"---
layout: post
title: some great title
created: 2029-12-18
---

some incredible post body with
multiple
lines
and paragraphs";

        let p = crate::parse_post(post_text).unwrap();

        assert_eq!(p.title, "some great title");
        assert_eq!(
            p.created_on,
            chrono::NaiveDate::parse_from_str("2029-12-18", "%Y-%m-%d").unwrap(),
        );
        assert_eq!(
            p.body.0,
            crate::md_to_html(
                "some incredible post body with
multiple
lines
and paragraphs"
            )
            .0
        )
    }

    #[test]
    fn recognizes_a_page() {
        let page_text = r"---
title: some great title
---

some incredible page body with
multiple
lines
and paragraphs";

        let p = crate::parse_page(page_text).unwrap();

        assert_eq!(p.title, "some great title");
        assert_eq!(
            p.body.0,
            crate::md_to_html(
                "some incredible page body with
multiple
lines
and paragraphs"
            )
            .0
        )
    }
}
