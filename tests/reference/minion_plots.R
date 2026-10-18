# Optional issue #14 capture: run unmodified pinned MinIONQC, capturing only its
# two MinION point families. Does not change/reclassify the #12 differential suite.
# Env: PG_MINION_SOURCE, PG_MINION_CAPTURE, PG_MINION_RENDER (default false).
suppressPackageStartupMessages(library(ggplot2))
suppressPackageStartupMessages(library(jsonlite))
stopifnot(getRversion()=="4.4.1",packageVersion("ggplot2")=="3.5.1",
          packageVersion("mgcv")=="1.9-1")
plots <- list()
original_save <- ggplot2::ggsave
counts <- function(index) {
  tab <- table(index)
  if(!length(tab)) return(matrix(numeric(),0,2))
  out <- cbind(as.numeric(names(tab)),as.numeric(tab))
  out[order(out[,1]),,drop=FALSE]
}
pixel <- function(x,n) pmin(n-1,floor(pmax(0,pmin(1,x))*n))
fraction <- function(x) if(min(x)==max(x)) rep(.5,length(x)) else (x-min(x))/(max(x)-min(x))
ggsave <- function(filename,plot,...) {
  family <- sub("\\.[^.]+$","",basename(filename))
  if(!family %in% c("flowcell_overview","length_vs_q")) return(invisible(filename))
  built <- ggplot_build(plot)
  raw <- plot$data
  positive <- raw$sequence_length_template>0
  d <- raw[positive,]
  x <- if(family=="flowcell_overview") d$start_time else log10(d$sequence_length_template)
  y <- if(family=="flowcell_overview") log10(d$sequence_length_template) else d$mean_qscore_template
  width <- if(family=="flowcell_overview") 64 else 512
  height <- if(family=="flowcell_overview") 40 else 256
  ix <- pixel(fraction(x),width);iy <- height-1-pixel(fraction(y),height)
  index <- if(family=="flowcell_overview")
    ((d$row-1)*height+iy)*1024+(d$col-1)*width+ix else iy*width+ix
  scale <- built$plot$scales$get_scales("colour")
  data <- built$data[[1]]
  by_channel <- lapply(sort(unique(raw$channel)),function(ch) {
    rows <- raw[raw$channel==ch,];p <- rows[rows$sequence_length_template>0,]
    list(channel=ch,eligible_reads=nrow(p),zero_length_records=sum(rows$sequence_length_template==0),
      row=rows$row[1]-1,col=rows$col[1]-1,
      length_range=if(nrow(p))range(p$sequence_length_template) else NULL,
      q_range=if(nrow(p))range(p$mean_qscore_template) else NULL)
  })
  plots[[family]] <<- list(summary_records=nrow(raw),eligible_reads=nrow(d),
    zero_length_records=sum(!positive),counts=counts(index),channels=by_channel,
    source_order="Actual MinIONQC plot$data order after add_cols merges; not input TSV order",
    alpha=unique(data$alpha),point_size=unique(data$size),
    x_transform=built$plot$scales$get_scales("x")$trans$name,
    y_transform=built$plot$scales$get_scales("y")$trans$name,
    color_transform=scale$trans$name,color_name=plot$labels$colour,
    color_transform_test=scale$trans$transform(c(1,exp(1))),
    layer_rows=nrow(data),nonfinite_x=sum(!is.finite(data$x)),nonfinite_y=sum(!is.finite(data$y)),
    missing_event_colors=if(family=="length_vs_q")unique(data$colour[is.na(raw$events_per_base)]) else NULL,
    zero_event_colors=if(family=="length_vs_q")unique(data$colour[!is.na(raw$events_per_base)&raw$events_per_base==0]) else NULL,
    positive_events_range=if(any(d$events_per_base>0,na.rm=TRUE))range(d$events_per_base[d$events_per_base>0],na.rm=TRUE) else NULL,
    missing_events_reads=sum(is.na(d$events_per_base)),zero_events_reads=sum(d$events_per_base==0,na.rm=TRUE),
    time_range_seconds=range(d$start_time),length_range_bp=range(d$sequence_length_template),q_range=range(d$mean_qscore_template))
  if(Sys.getenv("PG_MINION_RENDER","false")=="true")
    original_save(filename=filename,plot=plot,...,dpi=75)
  invisible(filename)
}
warnings <- character();error <- NULL
withCallingHandlers(tryCatch(source(Sys.getenv("PG_MINION_SOURCE"),local=globalenv()),
  error=function(e) error <<- conditionMessage(e)),warning=function(w){warnings <<- c(warnings,conditionMessage(w));invokeRestart("muffleWarning")})
write_json(list(error=error,warnings=unique(warnings),
  r=as.character(getRversion()),ggplot2=as.character(packageVersion("ggplot2")),
  session=capture.output(sessionInfo()),plots=plots),Sys.getenv("PG_MINION_CAPTURE"),
  auto_unbox=TRUE,digits=17,pretty=TRUE,na="null",null="null")
if(!is.null(error))quit(status=1)
