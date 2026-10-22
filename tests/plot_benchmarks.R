#!/usr/bin/env Rscript
# Optional documentation plots; reads recorded Markdown, never reruns a benchmark.
# Requires ggplot2 and an R build with Cairo. No ONTRunQC runtime dependency.
suppressPackageStartupMessages(library(ggplot2))
library(grid)

args <- commandArgs(trailingOnly = TRUE)
if (length(args) > 2L) stop("Usage: Rscript tests/plot_benchmarks.R [output-directory] [preview-directory]")
output <- if (length(args)) args[1] else "docs/figures/benchmark"
preview <- if (length(args) > 1L) args[2] else NULL
source_file <- "docs/benchmark.md"
lines <- readLines(source_file, warn = FALSE)

# Fail on missing/ambiguous tables or malformed numbers instead of stale plots.
read_table <- function(header) {
  start <- which(lines == header)
  if (length(start) != 1L) stop("Expected one table: ", header)
  cells <- function(line) trimws(strsplit(line, "|", fixed = TRUE)[[1]][-1])
  names <- cells(header)
  rows <- list()
  i <- start + 2L
  while (i <= length(lines) && startsWith(lines[i], "|")) {
    row <- cells(lines[i])
    if (length(row) != length(names)) stop("Malformed table row: ", lines[i])
    rows[[length(rows) + 1L]] <- row
    i <- i + 1L
  }
  if (!length(rows)) stop("Empty table: ", header)
  setNames(as.data.frame(do.call(rbind, rows), stringsAsFactors = FALSE), names)
}
number <- function(value) {
  result <- as.numeric(gsub(",", "", value, fixed = TRUE))
  if (any(!is.finite(result) | result < 0)) stop("Invalid measurement: ", paste(value, collapse = ", "))
  result
}
pair <- function(value) {
  parts <- strsplit(value, "/", fixed = TRUE)
  if (any(lengths(parts) != 2L)) stop("Expected before/after pair")
  t(vapply(parts, function(x) number(trimws(x)), numeric(2)))
}
colours <- c("Before" = "#94a3b8", "After" = "#4f46e5",
             "Plain" = "#94a3b8", "gzip" = "#4f46e5",
             "Metadata" = "#64748b", "SHA-256" = "#4f46e5")
base_theme <- theme_minimal(base_size = 11, base_family = "sans") +
  theme(plot.title = element_text(size = 12, face = "bold", colour = "#0f172a"),
        text = element_text(colour = "#334155"),
        panel.grid.minor = element_blank(), panel.grid.major.y = element_blank(),
        legend.title = element_blank(), legend.position = "bottom",
        plot.margin = margin(10, 18, 6, 8),
        axis.title.y = element_blank())

bars <- function(labels, values, groups, title, unit, digits = 2) {
  data <- data.frame(label = factor(labels, levels = rev(unique(labels))), value = values,
                     group = factor(groups, levels = unique(groups)))
  ggplot(data, aes(x = value, y = label, fill = group)) +
    geom_col(position = position_dodge(width = .76), width = .6) +
    geom_text(aes(label = formatC(value, format = "f", digits = digits, big.mark = ",")),
              position = position_dodge(width = .76), hjust = -.12, size = 3.1) +
    scale_fill_manual(values = colours) +
    scale_x_continuous(limits = c(0, NA), expand = expansion(mult = c(0, .24))) +
    labs(title = title, x = unit) + base_theme
}
paired_bars <- function(labels, values, title, unit, digits = 2, groups = c("Before", "After")) {
  stopifnot(ncol(values) == 2, nrow(values) == length(labels))
  bars(rep(labels, each = 2), as.vector(t(values)), rep(groups, times = length(labels)),
       title, unit, digits)
}

# Cairo embeds glyph paths: these standalone SVGs need no system fonts/network.
save_panels <- function(name, title, subtitle, plots, columns, width, height, note) {
  draw <- function() {
    grid.newpage()
    grid.text(title, x = .035, y = .96, just = "left",
              gp = gpar(fontsize = 19, fontface = "bold", col = "#0f172a"))
    grid.text(subtitle, x = .035, y = .915, just = "left",
              gp = gpar(fontsize = 10.5, col = "#475569"))
    rows <- ceiling(length(plots) / columns)
    pushViewport(viewport(x = .5, y = .49, width = .96, height = .77,
                          layout = grid.layout(rows, columns)))
    for (i in seq_along(plots)) {
      print(plots[[i]], newpage = FALSE,
            vp = viewport(layout.pos.row = ceiling(i / columns),
                          layout.pos.col = (i - 1L) %% columns + 1L))
    }
    popViewport()
    grid.text(note, x = .035, y = .045, just = "left",
              gp = gpar(fontsize = 9, col = "#475569"))
  }
  dir.create(output, recursive = TRUE, showWarnings = FALSE)
  path <- file.path(output, paste0(name, ".svg"))
  svg(path, width = width, height = height, bg = "white")
  draw()
  dev.off()
  content <- paste(readLines(path, warn = FALSE), collapse = "\n")
  # Cairo surface IDs depend on earlier graphics devices, including previews.
  # Rename identifiers/references only; leave paths, coordinates and data intact.
  ids <- regmatches(content, gregexpr('id="[^"]+"', content))[[1]]
  ids <- sub('^id="(.*)"$', '\\1', ids)
  for (i in seq_along(ids)) {
    stable <- paste0("asset-", i)
    content <- gsub(paste0('id="', ids[i], '"'), paste0('id="', stable, '"'), content, fixed = TRUE)
    content <- gsub(paste0('href="#', ids[i], '"'), paste0('href="#', stable, '"'), content, fixed = TRUE)
    content <- gsub(paste0('url(#', ids[i], ')'), paste0('url(#', stable, ')'), content, fixed = TRUE)
  }
  escape <- function(x) gsub("<", "&lt;", gsub("&", "&amp;", x, fixed = TRUE), fixed = TRUE)
  content <- sub("(<svg[^>]*>)", paste0('\\1\n<title>', escape(title), '</title>\n<desc>',
                escape(paste(subtitle, note)), '</desc>'), content, perl = TRUE)
  writeLines(content, path)
  if (!is.null(preview)) {
    dir.create(preview, recursive = TRUE, showWarnings = FALSE)
    png(file.path(preview, paste0(name, ".png")), width = width, height = height,
        units = "in", res = 130, type = "cairo", bg = "white")
    draw()
    dev.off()
  }
  message(path)
}

html <- read_table("| Structural measure | v0.4.0 | v0.4.1 |")
browser <- read_table("| Browser scenario | Load + two animation frames (ms) | Median synchronous handler (ms) | Maximum handler (ms) |")
select <- function(label) {
  rows <- html[html[[1]] == label, ]
  if (nrow(rows) != 1L) stop("Missing structural metric: ", label)
  matrix(number(unlist(rows[2:3])), nrow = 1)
}
stopifnot(nrow(html) == 10L, nrow(browser) == 5L,
          identical(browser[[1]][1:2], c("v0.4.0, JS, 1440px", "v0.4.1, JS, 1440px")))
plots <- list(
  paired_bars("HTML", select("HTML bytes") / 2^20, "Report size", "MiB"),
  paired_bars("SVG roots", select("SVG roots"), "SVG roots", "Count", 0),
  paired_bars("DOM elements", select("All DOM elements after JS initialization (1440px)"), "Mounted DOM · desktop JS", "Count", 0),
  paired_bars("Table rows", select("All DOM table rows after JS initialization (1440px)"), "Mounted table rows · desktop JS", "Count", 0),
  paired_bars("Load", matrix(number(browser[1:2, 2]), nrow = 1), "Load + two frames · 1440 px", "Milliseconds", 1),
  paired_bars("Handler", matrix(number(browser[1:2, 3]), nrow = 1), "Median table handler · 1440 px", "Milliseconds", 1))
save_panels("html-maintenance", "HTML report maintenance", "2026-10-17 · macOS arm64 · Chrome 155 · identical fictional 20,000-record input", plots, 3, 14, 8.5,
            "Before: v0.4.0; after: v0.4.1. One browser pass; no uncertainty estimate.\nAll source rows remain available without JavaScript; scientific JSON values are unchanged.")

minion <- read_table("| Input | Bytes | Positive-length reads | CLI wall before /after (s) | Peak RSS before /after (MiB) |")
stopifnot(nrow(minion) == 5L)
labels <- c("Synthetic · 150,002 reads", "Public MinION · 237,702", "Public 1D2 · 159,067",
            "Public short · 98", "Public RNA · 689")
stopifnot(identical(number(minion[[3]]), c(150002, 237702, 159067, 98, 689)))
plots <- list(paired_bars(labels, pair(minion[[4]]), "End-to-end CLI wall time", "Seconds", 3),
              paired_bars(labels, pair(minion[[5]]), "Peak child RSS", "MiB"))
save_panels("minion-full-count", "Cost of complete-data MinION visualization", "2026-10-18 · macOS arm64 · release build · same input within each pair", plots, 2, 14, 7,
            "Before: at most 16 sampled reads/channel; after: every eligible read contributes.\nCoverage increased; these measurements are not a speed or file-size optimization claim. One execution per configuration.")

exports <- read_table("| Format | Synthetic | Combined | Short MinION | Dense MinION | Dense figure MiB |")
stopifnot(nrow(exports) == 6L)
formats <- exports[[1]]
dense <- pair(exports[[5]])
size <- data.frame(label = factor(formats, levels = rev(formats)), value = number(exports[[6]]))
size_plot <- ggplot(size, aes(x = value, y = label)) +
  geom_point(colour = "#4f46e5", size = 3) +
  geom_text(aes(label = sprintf("%.2f", value)), hjust = -.25, size = 3.1) +
  scale_x_log10(breaks = c(1, 10, 100, 1000), limits = c(1, 1600)) +
  labs(title = "Requested figure bytes only", x = "Total figure size (MiB, log scale)") + base_theme
plots <- list(bars(formats, dense[, 1], rep("After", 6), "End-to-end CLI wall time", "Seconds", 3) + guides(fill = "none"),
              bars(formats, dense[, 2], rep("After", 6), "Peak child RSS", "MiB") + guides(fill = "none"), size_plot)
save_panels("static-exports", "Native static export formats", "2026-10-18 · macOS arm64 · dense public MinION input · 87 figures across two runs + pooled report", plots, 3, 14, 7,
            "Wall time and RSS cover the whole CLI; size excludes JSON/HTML. One execution per format.\nPDF retains vector content; PS is raster-backed at 300 DPI. JPEG is lossy; BMP is uncompressed.")

workers <- read_table("| Case | Mode / workers | Wall s | CPU user /sys s | RSS MiB | Temp MiB | File passes | Output bytes |")
workers <- workers[grepl("^(normal|strong) /", workers[[2]]), ]
stopifnot(nrow(workers) == 18L)
workers$mode <- ifelse(startsWith(workers[[2]], "normal"), "Metadata", "SHA-256")
workers$count <- number(sub(".*/", "", workers[[2]]))
plots <- list()
for (metric in c(3, 5)) {
  for (case in c("large", "dense", "public-minion")) {
    data <- workers[workers[[1]] == case, ]
    data$value <- number(data[[metric]])
    stopifnot(all(table(data$mode) == 3L), identical(sort(unique(data$count)), c(1, 2, 4)))
    label <- switch(case, large = "Synthetic · 1.2M records", dense = "Dense MinION · 4 × 4,100", "Public MinION · 608,208")
    plots[[length(plots) + 1L]] <- ggplot(data, aes(x = count, y = value, colour = mode, shape = mode)) +
      geom_line(linewidth = .7) + geom_point(size = 2.6) +
      scale_colour_manual(values = colours) + scale_shape_manual(values = c(16, 17)) +
      scale_x_continuous(breaks = c(1, 2, 4)) +
      scale_y_continuous(limits = c(0, NA), expand = expansion(mult = c(0, .12))) +
      labs(title = label, x = "Configured workers", y = if (metric == 3) "Wall time (seconds)" else "Peak child RSS (MiB)") + base_theme +
      theme(axis.title.y = element_text(angle = 90, margin = margin(r = 8)),
            panel.grid.major.y = element_line(colour = "#e2e8f0"))
  }
}
save_panels("workers-and-integrity", "Workers and source-integrity verification", "2026-10-19 · macOS arm64 · final guarded path · comparisons stay within each input", plots, 3, 14, 9,
            "SHA-256 adds hashing and publication rereads. Public input has two independent components, even at four configured workers.\nSingle executions, uncontrolled caches/load; lines only join measured configurations. More workers need not improve runtime.")

gzip <- read_table("| Mode | Input | Wall (s) | User (s) | System (s) | Peak RSS (MiB) | Passes | Temp peak (MiB) | Output bytes |")
stopifnot(nrow(gzip) == 6L, identical(gzip[[2]], rep(c("plain", "gzip"), 3)))
text <- paste(lines, collapse = " ")
match <- regmatches(text, regexec("Plain TSV: \\*\\*([0-9,]+) bytes\\*\\*; gzip: \\*\\*([0-9,]+) bytes\\*\\*", text))[[1]]
if (length(match) != 3L) stop("Missing source sizes")
labels <- c("Default · 1 pass", "Fits/KDE + SHA · 3 passes", "Dense MinION + SHA · 3 passes")
stopifnot(identical(number(gzip[[7]]), c(1, 1, 3, 3, 3, 3)))
plots <- list(
  paired_bars("Same 1M-record summary", matrix(number(match[2:3]) / 2^20, nrow = 1), "Source storage", "MiB", 2, c("Plain", "gzip")),
  paired_bars(labels, matrix(number(gzip[[3]]), ncol = 2, byrow = TRUE), "End-to-end CLI wall time", "Seconds", 3, c("Plain", "gzip")),
  paired_bars(labels, matrix(number(gzip[[4]]) + number(gzip[[5]]), ncol = 2, byrow = TRUE), "Child CPU · user + system", "CPU seconds", 3, c("Plain", "gzip")),
  paired_bars(labels, matrix(number(gzip[[6]]), ncol = 2, byrow = TRUE), "Peak child RSS", "MiB", 2, c("Plain", "gzip")))
save_panels("gzip-input", "Plain TSV and streaming gzip", "2026-10-19 · macOS arm64 · identical synthetic 1,000,000-record summaries", plots, 2, 14, 8.5,
            "Scientific JSON/HTML match within each pair; no temporary TSV is extracted. Every gzip pass decompresses again.\nOne execution/configuration; background compilation overlapped part of capture. This does not establish a universal gzip speedup.")


# The #20 table deliberately retains different workload sizes in every label;
# these bars are observations, never normalized speedup estimates.
temporal <- read_table("| Temporal case | Wall s | User CPU s | System CPU s | Peak RSS MiB | Sampled temp MiB | Input passes per source | Output bytes |")
stopifnot(nrow(temporal)==9L)
colours <- c(colours, Native="#94a3b8", Reference="#4f46e5")
labels <- c("1M plain · native","1M plain · reference","1M gzip · native","1M gzip · reference",
            "2M mixed · native","2M mixed · 1 worker","2M mixed · 2 workers","2M mixed · SHA","2M mixed · SVG")
groups <- ifelse(grepl("native$",temporal[[1]]),"Native","Reference")
plots <- list(bars(labels,number(temporal[[2]]),groups,"End-to-end CLI wall time","Seconds",3),
              bars(labels,number(temporal[[5]]),groups,"Peak child RSS","MiB"),
              bars(labels,number(temporal[[6]]),groups,"Sampled temporary storage","MiB · lower bound"),
              bars(labels,number(temporal[[8]])/2^20,groups,"JSON/HTML + requested SVG bytes","MiB"))
save_panels("temporal-reference","Cost of reference-compatible temporal plots",
            "2026-10-20 · macOS arm64 · synthetic data · 1M records per independent source",plots,2,14,10.5,
            "One execution/configuration; uncontrolled caches/load. 2M cases combine one plain and one gzip source.\nAll canonical statistics match; scratch is numeric, not extracted TSV. Temp peaks sampled at 10 ms include staged outputs.")
yield <- read_table("| Yield case | Version | Wall s | User CPU s | System CPU s | Peak RSS MiB | Sampled temp MiB | HTML bytes | JSON bytes | SVG bytes |")
stopifnot(nrow(yield)==8L,identical(yield[[2]],rep(c("before","after"),4)))
labels <- c("1M plain + SVG","1M gzip","2M mixed · 1 worker","2M mixed · 2 workers")
plots <- list(
  paired_bars(labels,matrix(number(yield[[3]]),ncol=2,byrow=TRUE),"End-to-end CLI wall time","Seconds",3),
  paired_bars(labels,matrix(number(yield[[6]]),ncol=2,byrow=TRUE),"Peak child RSS","MiB",2),
  paired_bars(labels,matrix((number(yield[[8]])+number(yield[[9]])+number(yield[[10]]))/2^20,ncol=2,byrow=TRUE),"HTML + JSON + requested SVG bytes","MiB",2))
save_panels("yield-length","Cost of error-aware retained-yield displays",
            "2026-10-22 · macOS arm64 · identical fictional inputs · before 876bf65 / after #21",plots,3,14,7,
            "One execution/configuration; uncontrolled caches/load. Exact non-display JSON values match. No additional input pass or disk scratch.\nHigher-fidelity bounded vertices increase serialized data; this is an accuracy/output-size trade-off, not a performance optimization.")
message("Rendered recorded observations only. R ", getRversion(), "; ggplot2 ", packageVersion("ggplot2"))
