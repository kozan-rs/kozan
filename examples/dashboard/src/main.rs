//! Dashboard — shadcn-inspired dark theme with scrollable sidebar.
//!
//! ALL styling lives in `dashboard.css`. Rust code only does structure + behavior.
//! Compare side-by-side with `dashboard.html` in Chrome.

use std::time::Duration;

use kozan::prelude::*;
use kozan::time::sleep;

fn main() -> kozan::Result<()> {
    let config = WindowConfig {
        title: "Kozan Dashboard".into(),
        width: 1100,
        height: 680,
        ..Default::default()
    };

    App::new().window(config, build_ui).run()
}

// ── Root ─────────────────────────────────────────────────────

fn build_ui(ctx: &ViewContext) {
    ctx.register_font(include_bytes!("../assets/Cairo.ttf") as &[u8]);

    let doc = ctx.document();
    doc.add_stylesheet(include_str!("../assets/dashboard.css"));

    let shell = doc.div();
    shell.class_add("shell");
    doc.body().child(shell);

    let fps_badge = build_fps_overlay(doc, ctx);
    doc.body().child(fps_badge);

    shell.child(build_header(doc));

    let body = doc.div();
    body.class_add("body-row");
    shell.child(body);

    body.child(build_sidebar(doc));
    body.child(build_content(doc, ctx));
}

// ── FPS overlay ──────────────────────────────────────────────

fn build_fps_overlay(doc: &Document, ctx: &ViewContext) -> HtmlDivElement {
    let badge = doc.div();
    badge.class_add("fps-badge");

    let text = doc.create_text("-- FPS");
    badge.append(text);

    ctx.request_frame(move |info| {
        let t = info.prev_timing;
        text.set_content(format!(
            "{:.0} FPS | {:.1}ms  S={:.1} L={:.1} P={:.1}",
            info.fps, t.total_ms, t.style_ms, t.layout_ms, t.paint_ms,
        ));
        true
    });

    badge
}

// ── Header ───────────────────────────────────────────────────

fn build_header(doc: &Document) -> HtmlDivElement {
    let header = doc.div();
    header.class_add("header");

    let logo = doc.div();
    logo.class_add("logo");
    let logo_inner = doc.div();
    logo_inner.class_add("logo-inner");
    header.child(logo.child(logo_inner));

    header.append(doc.create_text("Kozan"));

    let spacer = doc.div();
    spacer.class_add("spacer");
    header.child(spacer);

    let label = doc.div();
    label.class_add("header-label");
    label.append(doc.create_text("Dashboard"));
    header.child(label);

    let status = doc.div();
    status.class_add("status-dot");
    status.class_add("status-active");
    header.child(status);

    header
}

// ── Sidebar ──────────────────────────────────────────────────

struct NavItem {
    icon: &'static str,
    label: &'static str,
    active: bool,
}

const NAV_MAIN: &[NavItem] = &[
    NavItem { icon: "blue",   label: "Dashboard",     active: true },
    NavItem { icon: "purple", label: "Analytics",      active: false },
    NavItem { icon: "teal",   label: "Reports",        active: false },
    NavItem { icon: "orange", label: "Calendar",       active: false },
    NavItem { icon: "green",  label: "Projects",       active: false },
    NavItem { icon: "cyan",   label: "Tasks",          active: false },
    NavItem { icon: "pink",   label: "Messages",       active: false },
    NavItem { icon: "amber",  label: "Notifications",  active: false },
];

const NAV_SETTINGS: &[NavItem] = &[
    NavItem { icon: "zinc",   label: "General",        active: false },
    NavItem { icon: "indigo", label: "Team",           active: false },
    NavItem { icon: "orange", label: "Billing",        active: false },
    NavItem { icon: "rose",   label: "Integrations",   active: false },
    NavItem { icon: "teal",   label: "API Keys",       active: false },
    NavItem { icon: "purple", label: "Security",       active: false },
    NavItem { icon: "sky",    label: "Appearance",     active: false },
];

const NAV_SUPPORT: &[NavItem] = &[
    NavItem { icon: "blue",   label: "Documentation",  active: false },
    NavItem { icon: "lime",   label: "Changelog",      active: false },
    NavItem { icon: "green",  label: "Help Center",    active: false },
    NavItem { icon: "cyan",   label: "Community",      active: false },
    NavItem { icon: "amber",  label: "Feedback",       active: false },
];

fn build_sidebar(doc: &Document) -> HtmlDivElement {
    let sidebar = doc.div();
    sidebar.class_add("sidebar");

    sidebar.child(build_sidebar_header(doc));
    sidebar.child(build_sidebar_nav(doc));
    sidebar.child(build_sidebar_footer(doc));

    sidebar
}

fn build_sidebar_header(doc: &Document) -> HtmlDivElement {
    let header = doc.div();
    header.class_add("sidebar-header");

    let avatar = doc.div();
    avatar.class_add("avatar");
    let avatar_inner = doc.div();
    avatar_inner.class_add("avatar-inner");
    header.child(avatar.child(avatar_inner));

    let info = doc.div();
    info.class_add("avatar-info");

    let name = doc.div();
    name.class_add("avatar-name");
    name.append(doc.create_text("Kozan User"));
    info.child(name);

    let email = doc.div();
    email.class_add("avatar-email");
    email.append(doc.create_text("user@kozan.dev"));
    info.child(email);

    header.child(info);
    header
}

fn build_sidebar_nav(doc: &Document) -> HtmlDivElement {
    let nav = doc.div();
    nav.class_add("sidebar-nav");

    nav.child(build_nav_section(doc, "MAIN", NAV_MAIN));
    nav.child(build_nav_section(doc, "SETTINGS", NAV_SETTINGS));
    nav.child(build_nav_section(doc, "SUPPORT", NAV_SUPPORT));

    nav
}

fn build_nav_section(doc: &Document, title: &str, items: &[NavItem]) -> HtmlDivElement {
    let section = doc.div();

    let label = doc.div();
    label.class_add("nav-section");
    label.append(doc.create_text(title));
    section.child(label);

    for item in items {
        section.child(build_nav_item(doc, item));
    }

    section
}

fn build_nav_item(doc: &Document, nav: &NavItem) -> HtmlDivElement {
    let item = doc.div();
    item.class_add("nav-item");
    if nav.active { item.class_add("nav-active"); }

    let icon = doc.div();
    icon.class_add("nav-icon");
    icon.class_add(&format!("icon-{}", nav.icon));
    item.child(icon);

    item.append(doc.create_text(nav.label));

    item.on::<ClickEvent>(move |_evt, _ctx| {
        if let Some(grandparent) = item.parent().and_then(|p| p.parent()) {
            for section in grandparent.children() {
                for child in section.children() {
                    child.class_remove("nav-active");
                }
            }
        }
        item.class_add("nav-active");
    });

    item
}

fn build_sidebar_footer(doc: &Document) -> HtmlDivElement {
    let footer = doc.div();
    footer.class_add("sidebar-footer");

    let badge = doc.div();
    badge.class_add("sidebar-badge");

    let dot = doc.div();
    dot.class_add("badge-dot");
    badge.child(dot);

    let label = doc.div();
    label.class_add("badge-label");
    label.append(doc.create_text("All systems online"));
    badge.child(label);

    footer.child(badge);
    footer
}

// ── Content ──────────────────────────────────────────────────

fn build_content(doc: &Document, ctx: &ViewContext) -> HtmlDivElement {
    let content = doc.div();
    content.class_add("content");

    content.child(build_cards_row(doc, ctx));
    content.child(build_chart_panel(doc, ctx));
    content.child(build_activity_panel(doc));

    content
}

// ── Cards ────────────────────────────────────────────────────

struct CardSpec {
    accent: &'static str,
    value: &'static str,
    label: &'static str,
    delay_ms: u64,
    fill: f32,
}

const CARDS: &[CardSpec] = &[
    CardSpec { accent: "blue",   value: "$2,847", label: "Revenue",  delay_ms: 400,  fill: 0.78 },
    CardSpec { accent: "purple", value: "1,024",  label: "Users",    delay_ms: 600,  fill: 0.52 },
    CardSpec { accent: "teal",   value: "98.2%",  label: "Uptime",   delay_ms: 800,  fill: 0.91 },
    CardSpec { accent: "orange", value: "142",    label: "Issues",   delay_ms: 1000, fill: 0.35 },
];

fn build_cards_row(doc: &Document, ctx: &ViewContext) -> HtmlDivElement {
    let row = doc.div();
    row.class_add("cards-row");

    for spec in CARDS {
        let card = build_card(doc, spec);
        row.child(card);

        let accent = spec.accent;
        let fill_pct = spec.fill;
        let delay = spec.delay_ms;

        let card_class = format!("card-{accent}");
        let fill_class = format!("fill-{accent}");

        ctx.spawn(async move {
            sleep(Duration::from_millis(delay)).await;
            card.class_add(&card_class);

            let children = card.children();
            if let Some(icon) = children.first() {
                icon.class_add(&fill_class);
            }
            if let Some(track) = children.get(3) {
                if let Some(fill_bar) = track.first_child() {
                    fill_bar.class_add(&fill_class);
                    fill_bar.style().w(pct(fill_pct * 100.0));
                }
            }
        });
    }

    row
}

fn build_card(doc: &Document, spec: &CardSpec) -> HtmlDivElement {
    let card = doc.div();
    card.class_add("card");

    let icon = doc.div();
    icon.class_add("card-icon");
    icon.class_add(&format!("icon-{}", spec.accent));

    let value = doc.div();
    value.class_add("card-value");
    value.append(doc.create_text(spec.value));

    let label = doc.div();
    label.class_add("card-label");
    label.append(doc.create_text(spec.label));

    let track = doc.div();
    track.class_add("card-track");
    let fill = doc.div();
    fill.class_add("card-fill");
    track.child(fill);

    card.child(icon).child(value).child(label).child(track)
}

// ── Chart panel ──────────────────────────────────────────────

const CHART_HEIGHTS: [f32; 8] = [0.45, 0.72, 0.58, 0.95, 0.65, 0.40, 0.82, 0.55];

fn build_chart_panel(doc: &Document, ctx: &ViewContext) -> HtmlDivElement {
    let panel = doc.div();
    panel.class_add("panel");

    let header = doc.div();
    header.class_add("panel-header");

    let title = doc.div();
    title.class_add("panel-title");
    title.append(doc.create_text("Weekly Overview"));

    let spacer = doc.div();
    spacer.class_add("spacer");

    let badge = doc.div();
    badge.class_add("panel-badge");
    badge.append(doc.create_text("+12.5%"));

    header.child(title).child(spacer).child(badge);
    panel.child(header);

    let chart = doc.div();
    chart.class_add("chart");

    let mut fills = Vec::with_capacity(8);
    for _ in 0..8 {
        let col = doc.div();
        col.class_add("chart-col");

        let track = doc.div();
        track.class_add("chart-track");

        let fill = doc.div();
        fill.class_add("chart-fill");
        track.child(fill);
        col.child(track);

        let tick = doc.div();
        tick.class_add("chart-tick");
        col.child(tick);

        chart.child(col);
        fills.push(fill);
    }

    panel.child(chart);

    ctx.spawn(async move {
        sleep(Duration::from_millis(1200)).await;
        for (i, fill) in fills.iter().enumerate() {
            let color = if i == 3 { "fill-purple" } else { "fill-blue" };
            fill.class_add(color);
            fill.style().h(pct(CHART_HEIGHTS[i] * 100.0));
        }
    });

    panel
}

// ── Activity panel ───────────────────────────────────────────

struct Activity {
    color: &'static str,
    text: &'static str,
    time: &'static str,
}

const ACTIVITIES: &[Activity] = &[
    Activity { color: "green",  text: "Deployment succeeded",    time: "2m ago" },
    Activity { color: "blue",   text: "New user registered",     time: "5m ago" },
    Activity { color: "orange", text: "Payment processed",       time: "12m ago" },
    Activity { color: "purple", text: "Report generated",        time: "1h ago" },
    Activity { color: "rose",   text: "Alert triggered",         time: "2h ago" },
];

fn build_activity_panel(doc: &Document) -> HtmlDivElement {
    let panel = doc.div();
    panel.class_add("panel");

    let header = doc.div();
    header.class_add("panel-header");

    let title = doc.div();
    title.class_add("panel-title");
    title.append(doc.create_text("Recent Activity"));

    let spacer = doc.div();
    spacer.class_add("spacer");

    let subtitle = doc.div();
    subtitle.class_add("panel-subtitle");
    subtitle.append(doc.create_text("Last 24 hours"));

    header.child(title).child(spacer).child(subtitle);
    panel.child(header);

    for (i, activity) in ACTIVITIES.iter().enumerate() {
        if i > 0 {
            let divider = doc.div();
            divider.class_add("divider");
            panel.child(divider);
        }

        let row = doc.div();
        row.class_add("activity-row");

        let dot = doc.div();
        dot.class_add("activity-dot");
        dot.class_add(&format!("icon-{}", activity.color));
        row.child(dot);

        let text = doc.div();
        text.class_add("activity-text");
        text.append(doc.create_text(activity.text));
        row.child(text);

        let time = doc.div();
        time.class_add("activity-time");
        time.append(doc.create_text(activity.time));
        row.child(time);

        panel.child(row);
    }

    panel
}
