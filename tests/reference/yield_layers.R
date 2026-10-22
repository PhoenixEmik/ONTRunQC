# Execute unchanged pinned single.flowcell yield-by-length expressions. Small,
# fictional goldens; no fitted curves, public reads or private metadata in Git.
library(ggplot2)
library(jsonlite)
library(viridis)
stopifnot(as.character(getRversion())=="4.4.1",as.character(packageVersion("ggplot2"))=="3.5.1",as.character(packageVersion("mgcv"))=="1.9.1")
args <- commandArgs(TRUE)
source <- normalizePath(args[1])
revision <- system2("git",c("-C",shQuote(dirname(source)),"rev-parse","HEAD"),stdout=TRUE)
stopifnot(revision=="54485d586038d8ee36f168dd94f31c423f52f581")
stopifnot(!length(system2("git",c("-C",shQuote(dirname(source)),"status","--porcelain"),stdout=TRUE)))
assigned <- function(e) if(is.call(e) && as.character(e[[1]]) %in% c("=","<-") && is.symbol(e[[2]])) as.character(e[[2]]) else ""
for(e in parse(source)) if(assigned(e) %in% c(paste0("p",1:8),paste0("q",1:8),"map","add_cols","single.flowcell")) eval(e)
body <- as.list(body(single.flowcell))[-1]
labels <- vapply(body,assigned,character(1))
cases <- list(
 singleton=list(length=c(10),q=c(7),cutoff=7),
 identical=list(length=c(20,20,20,20),q=c(6,7,9,12),cutoff=7),
 irregular_ties=list(length=c(0,10,20,20,21,100),q=c(7,6,7,8,7,9),cutoff=7),
 long_tail=list(length=c(1,2,3,10000,1000000),q=c(7,7,6,8,9),cutoff=7),
 signed=list(length=c(0,10,20,20),q=c(-3,-2,0,2),cutoff=-2),
 zero=list(length=c(0,0),q=c(0,7),cutoff=7))
for(name in names(cases)) {
  c <- cases[[name]]
  env <- new.env(parent=globalenv())
  env$q_title <- paste0("Q>=",c$cutoff)
  raw <- data.frame(start_time=(seq_along(c$length)%%3)*3600,sequence_length_template=c$length,mean_qscore_template=c$q,channel=1)
  a <- add_cols(raw,-Inf); a$Q_cutoff <- "All reads"
  b <- add_cols(raw,c$cutoff); b$Q_cutoff <- env$q_title
  env$d <- rbind(a,b)
  eval(body[[which(labels=="p6")[1]]],env)
  eval(body[[which(labels=="xmax")[1]]],env)
  c$axis_maximum_bp <- if(is.finite(env$xmax)) env$xmax else NULL
  if(is.finite(env$xmax)) {
    eval(body[[which(labels=="p6")[2]]],env)
    built <- suppressWarnings(ggplot_build(env$p6))
    c$layer <- built$data[[1]][,c("x","y","group")]
    c$population_labels <- built$plot$scales$get_scales("colour")$get_labels()
  } else c$unavailable <- "Original strict 1%-yield crop has no finite maximum for zero yield."
  cases[[name]] <- c
}
write_json(list(reference=list(commit=revision,r=as.character(getRversion()),ggplot2=as.character(packageVersion("ggplot2")),mgcv=as.character(packageVersion("mgcv"))),cases=cases),args[2],digits=NA,auto_unbox=TRUE,pretty=TRUE,na="null")
