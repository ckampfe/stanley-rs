use glob::glob;
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::line_ending;
use nom::combinator::rest;
use nom::sequence::{preceded, terminated};
use nom::*;
use pulldown_cmark::{html, Parser};
use rss::{ChannelBuilder, ItemBuilder};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{env, fs};
use tera::{Context, Tera};

#[derive(Clone, Debug)]
struct Post<'a> {
    title: &'a str,
    created_on: time::Tm,
    body: String,
}

impl<'a> Post<'a> {
    fn new(title: &'a str, created_on: time::Tm, body: String) -> Self {
        Post {
            title,
            created_on,
            body,
        }
    }
}

#[derive(Clone, Debug)]
struct Page<'a> {
    title: &'a str,
    created_on: time::Tm,
    body: String,
}

impl<'a> Page<'a> {
    fn new(title: &'a str, created_on: time::Tm, body: String) -> Self {
        Page {
            title,
            created_on,
            body,
        }
    }
}

fn parse_md(markdown_str: &str) -> String {
    let parser = Parser::new(markdown_str);
    let mut html_buf = String::new();
    html::push_html(&mut html_buf, parser);
    html_buf
}

fn title(s: &[u8]) -> IResult<&[u8], &[u8]> {
    let (s, title) = preceded(tag("title: "), terminated(take_until("\n"), line_ending))(s)?;
    Ok((s, title))
}

fn created_on(s: &[u8]) -> IResult<&[u8], &[u8]> {
    let (s, created) = preceded(tag("created: "), terminated(take_until("\n"), line_ending))(s)?;
    Ok((s, created))
}

fn post(s: &[u8]) -> IResult<&[u8], Post> {
    let (s, _) = tag("---")(s)?;
    let (s, _) = line_ending(s)?;
    let (s, _) = tag("layout: post")(s)?;
    let (s, _) = line_ending(s)?;
    let (s, title) = title(s)?;
    let (s, created_on) = created_on(s)?;
    let (s, _) = tag("---")(s)?;
    let (s, body) = rest(s)?;

    let post = Post::new(
        std::str::from_utf8(title).unwrap(),
        time::strptime(std::str::from_utf8(created_on).unwrap(), "%Y-%m-%d").unwrap(),
        parse_md(std::str::from_utf8(body).unwrap()),
    );

    Ok((s, post))
}

fn page(s: &[u8]) -> IResult<&[u8], Page> {
    let (s, _) = tag("---")(s)?;
    let (s, _) = line_ending(s)?;
    let (s, title) = title(s)?;
    let (s, created_on) = created_on(s)?;
    let (s, _) = tag("---")(s)?;
    let (s, body) = rest(s)?;

    let page = Page::new(
        std::str::from_utf8(title).unwrap(),
        time::strptime(std::str::from_utf8(created_on).unwrap(), "%Y-%m-%d").unwrap(),
        parse_md(std::str::from_utf8(body).unwrap()),
    );

    Ok((s, page))
}

fn get_markdown_files(path: &Path) -> Result<glob::Paths, glob::PatternError> {
    let mdpath = path.join("**/*.md");
    let mdpathstr = mdpath.to_str().unwrap();
    glob(mdpathstr)
}

const PAGE: &str = r###"
<div>
  <h1>{{title}}</h1>
  <div class="page">{{content}}</div>
</div>
"###;

const POST: &str = r###"
<div>
    <h2>{{title}}</h2>
    <p class="meta">{{created}}</p>
    <div class="post">{{content}}</div>
</div>
"###;

const INDEX_LINK: &str = r###"
<li>
  <a href="{{filename}}">{{title}}</a>
  <span>{{created_at}}</span>
</li>
"###;

const INDEX: &str = r###"
<div id="home">
  <ul class="posts">
  {% for post_link in post_links %}
    {{post_link}}
  {% endfor %}
  </ul>
</div>
"###;

const LAYOUT: &str = r###"
<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8">
    <meta content="IE=edge,chrome=1" http-equiv="X-UA-Compatible">
    <title>
      {{title}}
    </title>
    <meta content="width=device-width" name="viewport">
    <link href="prism.css" rel="stylesheet">
    <link href="main.css" rel="stylesheet">
    <link href="/favicon" rel="icon" type="image/png">
  </head>
  <body>
    <div class="container">
      <div class="site">
        <div class="header">
          <h1 class="title">
            <a href="index.html">Clark Kampfe</a>
          </h1>
          <a class="extra" href="about.html">about</a>
          <a class="extra" href="resume.html">resumé</a>
        </div>
        {{content}}
        <div class="footer">
          <div class="contact">
            <p>
              <a href="https://github.com/ckampfe/">github</a>
              <a href="https://twitter.com/clarkkampfe">twitter</a>
            </p>
          </div>
        </div>
      </div>
    </div>
  </body>
</html>
"###;

fn rss_feed() -> rss::Channel {
    ChannelBuilder::default()
        .title("zct")
        .link("https://zeroclarkthiry.com")
        .description("zeroclarkthirty.com")
        .build()
        .unwrap()
}

fn rss_item(post: Post, link: &str) -> rss::Item {
    ItemBuilder::default()
        .title(post.title.to_string())
        .link(link.to_owned())
        .content(post.body)
        .build()
        .unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = env::current_dir()?;
    let build_dir = cwd.join("build");

    let mut reg = Tera::default();
    reg.add_raw_template("layout", &LAYOUT)?;
    reg.add_raw_template("index_link", &INDEX_LINK)?;
    reg.add_raw_template("index", &INDEX)?;
    reg.add_raw_template("post", &POST)?;
    reg.add_raw_template("page", &PAGE)?;

    let post_paths = get_markdown_files(&cwd.join("posts"))?;

    let mut feed = rss_feed();
    let mut rss_items = vec![];
    let mut index_links = vec![];
    let mut paths_and_content: Vec<(PathBuf, Vec<u8>)> = vec![];

    for post_path in post_paths {
        let pp = post_path?;
        let content = fs::read(&pp)?;
        paths_and_content.push((pp, content));
    }

    let mut paths_and_posts: Vec<(&PathBuf, Post)> = paths_and_content
        .iter()
        .map(|(post_path, content)| {
            let post = post(content).unwrap().1;
            (post_path, post)
        })
        .collect::<Vec<(&PathBuf, Post)>>();

    paths_and_posts.sort_unstable_by(|a, b| b.1.created_on.cmp(&a.1.created_on));

    for (post_path, post) in paths_and_posts {
        let mut post_data = Context::new();
        let mut layout_data = Context::new();
        let mut index_link_data = Context::new();

        let post_created_on = time::strftime("%Y-%m-%d", &post.created_on)?;
        post_data.insert("title", post.title);
        post_data.insert("created", &post_created_on);
        post_data.insert("content", &post.body);
        let post_html = reg.render("post", &post_data)?;

        layout_data.insert("title", post.title);
        layout_data.insert("content", &post_html);
        let post_layout_html = reg.render("layout", &layout_data)?;

        let filename = post_path
            .file_name()
            .ok_or_else(|| "Could not make post path into str")?;

        let mut post_output_path = PathBuf::new();
        post_output_path.push(&build_dir);
        post_output_path.push(filename);
        post_output_path.set_extension("html");
        let mut post_output = fs::File::create(post_output_path)?;
        post_output.write_all(post_layout_html.as_bytes())?;

        let mut index_link_post_path = PathBuf::new();

        index_link_post_path.push(filename);
        index_link_post_path.set_extension("html");

        let index_link_post_str = index_link_post_path
            .to_str()
            .ok_or_else(|| "Could not create filename from osstr")?;
        index_link_data.insert("title", post.title);
        index_link_data.insert("filename", index_link_post_str);
        index_link_data.insert("created_at", &post_created_on);
        let index_link_html = reg.render("index_link", &index_link_data)?;

        index_links.push(index_link_html);

        let mut post_link = PathBuf::new();
        post_link.push("https://zeroclarkthirty.com");
        post_link.push(filename);
        post_link.set_extension("html");
        let post_link_str = post_link
            .to_str()
            .ok_or_else(|| "Could not convert link to str")?;
        let post_rss_item = rss_item(post, post_link_str);
        rss_items.push(post_rss_item);
    }

    let mut index_data = Context::default();

    index_data.insert("post_links", &index_links);
    let index_html = reg.render("index", &index_data)?;

    let mut layout_data = Context::default();
    layout_data.insert("title", "Clark Kampfe - zeroclarkthirty.com");
    layout_data.insert("content", &index_html);
    let index_layout_html = reg.render("layout", &layout_data)?;

    let mut index_output_path = PathBuf::new();
    index_output_path.push(&build_dir);
    index_output_path.push("index");
    index_output_path.set_extension("html");
    let mut index_output = fs::File::create(index_output_path)?;
    index_output.write_all(index_layout_html.as_bytes())?;

    feed.set_items(rss_items);
    let mut rss_feed_path = PathBuf::new();
    rss_feed_path.push(&build_dir);
    rss_feed_path.push("feed");
    let feed_file = fs::File::create(rss_feed_path)?;

    feed.write_to(feed_file)?;

    let page_paths = get_markdown_files(&cwd.join("pages"))?;

    for page_path in page_paths {
        let pp = page_path?;
        let contents = fs::read(&pp)?;
        let (_, page) = page(&contents).unwrap();

        let mut page_data = Context::default();
        let page_created_on = time::strftime("%Y-%m-%d", &page.created_on)?;
        page_data.insert("title", page.title);
        page_data.insert("created", &page_created_on);
        page_data.insert("content", &page.body);
        let page_html = reg.render("page", &page_data)?;

        let mut layout_data = Context::default();
        layout_data.insert("title", page.title);
        layout_data.insert("content", &page_html);
        let page_layout_html = reg.render("layout", &layout_data)?;

        let filename = pp
            .file_name()
            .ok_or_else(|| "Could not make page path into str")?;
        let mut page_output_path = PathBuf::new();
        page_output_path.push(&build_dir);
        page_output_path.push(filename);
        page_output_path.set_extension("html");
        let mut page_output = fs::File::create(page_output_path)?;
        page_output.write_all(page_layout_html.as_bytes())?;
    }

    Ok(())
}
