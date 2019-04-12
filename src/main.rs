use glob::glob;
use handlebars::Handlebars;
use nom::*;
use pulldown_cmark::html;
use pulldown_cmark::Parser;
use rss::{ChannelBuilder, ItemBuilder};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug)]
enum Layout {
    Post,
    Page,
}
#[derive(Clone, Copy, Debug)]
struct Prelude<'a> {
    layout: Layout,
    title: &'a str,
    created_on: time::Tm,
}

#[derive(Clone, Debug)]
struct Post<'a> {
    prelude: Prelude<'a>,
    body: String,
}

impl<'a> Post<'a> {
    fn new(title: &'a str, created_on: time::Tm, body: String) -> Self {
        Post {
            prelude: Prelude {
                layout: Layout::Post,
                title: title,
                created_on: created_on,
            },
            body: body,
        }
    }
}

#[derive(Clone, Debug)]
struct Page<'a> {
    prelude: Prelude<'a>,
    body: String,
}

impl<'a> Page<'a> {
    fn new(title: &'a str, created_on: time::Tm, body: String) -> Self {
        Page {
            prelude: Prelude {
                layout: Layout::Page,
                title: title,
                created_on: created_on,
            },
            body: body,
        }
    }
}

fn parse_md(markdown_str: &str) -> String {
    let parser = Parser::new(markdown_str);
    let mut html_buf = String::new();
    html::push_html(&mut html_buf, parser);
    html_buf
}

named!(
    parse_post<Post>,
    do_parse!(
        tag!("---")
            >> line_ending
            >> tag!("layout: post")
            >> line_ending
            >> title:
                preceded!(
                    ws!(tag!("title:")),
                    terminated!(take_until!("\n"), line_ending)
                )
            >> created_on:
                preceded!(
                    ws!(tag!("created:")),
                    terminated!(take_until!("\n"), line_ending)
                )
            >> tag!("---")
            >> body: rest
            >> (Post::new(
                std::str::from_utf8(title).unwrap(),
                time::strptime(std::str::from_utf8(created_on).unwrap(), "%Y-%m-%d").unwrap(),
                parse_md(std::str::from_utf8(body).unwrap())
            ))
    )
);

named!(
    parse_page<Page>,
    do_parse!(
        tag!("---")
            >> line_ending
            >> title:
                preceded!(
                    ws!(tag!("title:")),
                    terminated!(take_until!("\n"), line_ending)
                )
            >> created_on:
                preceded!(
                    ws!(tag!("created:")),
                    terminated!(take_until!("\n"), line_ending)
                )
            >> tag!("---")
            >> body: rest
            >> (Page::new(
                std::str::from_utf8(title).unwrap(),
                time::strptime(std::str::from_utf8(created_on).unwrap(), "%Y-%m-%d").unwrap(),
                parse_md(std::str::from_utf8(body).unwrap())
            ))
    )
);

////////////////////////////////////////////////////////

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
  {{#each post_links as |post_link| ~}}
    {{post_link}}
  {{/each}}
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
        .title(post.prelude.title.to_string())
        .link(link.to_owned())
        .content(post.body.to_owned())
        .build()
        .unwrap()
}

fn main() -> Result<(), Box<std::error::Error>> {
    let cwd = env::current_dir()?;
    let build_dir = cwd.join("build");

    // register templates
    let mut reg = Handlebars::new();
    reg.register_escape_fn(handlebars::no_escape);
    reg.register_template_string("layout", &LAYOUT)?;
    reg.register_template_string("index_link", &INDEX_LINK)?;
    reg.register_template_string("index", &INDEX)?;
    reg.register_template_string("post", &POST)?;
    reg.register_template_string("page", &PAGE)?;

    // get posts
    let post_paths = get_markdown_files(&cwd.join("posts"))?;

    // initialize collections
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
            let post = parse_post(content).unwrap().1;
            (post_path, post)
        })
        .collect::<Vec<(&PathBuf, Post)>>();

    // sort posts descending
    paths_and_posts.sort_unstable_by(|a, b| b.1.prelude.created_on.cmp(&a.1.prelude.created_on));

    for (post_path, post) in paths_and_posts {
        // create post HTML
        let mut post_data = HashMap::new();
        post_data.insert("title", post.prelude.title);
        let post_created_on = time::strftime("%Y-%m-%d", &post.prelude.created_on)?;
        post_data.insert("created", &post_created_on);
        post_data.insert("content", &post.body);
        let post_html = reg.render("post", &post_data)?;

        // put post HTML in main layout HTML
        let mut layout_data = HashMap::new();
        layout_data.insert("title", post.prelude.title);
        layout_data.insert("content", &post_html);
        let post_layout_html = reg.render("layout", &layout_data)?;

        // write post to fs
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
        let mut index_link_data = HashMap::new();
        index_link_data.insert("title", post.prelude.title);
        index_link_data.insert("filename", index_link_post_str);
        index_link_data.insert("created_at", &post_created_on);
        let index_link_html = reg.render("index_link", &index_link_data)?;

        index_links.push(index_link_html);

        // create rss entry for post
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

    // build index
    let mut index_data = HashMap::new();

    index_data.insert("post_links", &index_links);
    let index_html = reg.render("index", &index_data)?;

    let mut layout_data = HashMap::new();
    layout_data.insert("title", "Clark Kampfe - zeroclarkthirty.com");
    layout_data.insert("content", &index_html);
    let index_layout_html = reg.render("layout", &layout_data)?;

    // write index to fs
    let mut index_output_path = PathBuf::new();
    index_output_path.push(&build_dir);
    index_output_path.push("index");
    index_output_path.set_extension("html");
    let mut index_output = fs::File::create(index_output_path)?;
    index_output.write_all(index_layout_html.as_bytes())?;

    // write RSS stuff to fs
    feed.set_items(rss_items);
    let mut rss_feed_path = PathBuf::new();
    rss_feed_path.push(&build_dir);
    rss_feed_path.push("feed");
    let feed_file = fs::File::create(rss_feed_path)?;

    feed.write_to(feed_file)?;

    let page_paths = get_markdown_files(&cwd.join("pages"))?;

    for page_path in page_paths {
        let pp = page_path?;
        let contents = &fs::read(&pp)?;
        let (_, page) = parse_page(contents).unwrap();

        let mut page_data = HashMap::new();
        page_data.insert("title", page.prelude.title);
        let page_created_on = time::strftime("%Y-%m-%d", &page.prelude.created_on)?;
        page_data.insert("created", &page_created_on);
        page_data.insert("content", &page.body);
        let page_html = reg.render("page", &page_data)?;

        // put page HTML in main layout HTML
        let mut layout_data = HashMap::new();
        layout_data.insert("title", page.prelude.title);
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
