# Synthetic plot-layer goldens: execute expressions from the unchanged pinned
# script, not a reimplementation of its temporal calculations.
library(ggplot2)
library(jsonlite)
library(viridis)
stopifnot(as.character(getRversion())=="4.4.1",as.character(packageVersion("ggplot2"))=="3.5.1",as.character(packageVersion("mgcv"))=="1.9.1")
args <- commandArgs(TRUE)
source <- normalizePath(args[1])
revision <- system2("git",c("-C",shQuote(dirname(source)),"rev-parse","HEAD"),stdout=TRUE)
stopifnot(revision=="54485d586038d8ee36f168dd94f31c423f52f581")
stopifnot(!length(system2("git",c("-C",shQuote(dirname(source)),"status","--porcelain"),stdout=TRUE)))
expressions <- parse(source)
assigned <- function(e) if(is.call(e) && as.character(e[[1]]) %in% c("=","<-") && is.symbol(e[[2]])) as.character(e[[2]]) else ""
for(e in expressions) if(assigned(e) %in% c(paste0("p",1:8),paste0("q",1:8),"map","add_cols","single.flowcell")) eval(e)
body <- as.list(body(single.flowcell))[-1]
labels <- vapply(body,assigned,character(1))
cases <- list(
 boundaries=list(time=c(7200,3599,0,7199,3600),length=c(30,0,10,40,20),q=c(7,-2,7,6,8),channel=c(1000,1001,1000,1002,1001),cutoff=7),
 missing=list(time=c(18000,3600,3601),length=c(100,20,40),q=c(7,7,-1),channel=c(1,2,1),cutoff=7),
 ties_minion=list(time=c(100,100,100,0,7200),length=c(10,30,20,0,50),q=c(7,8,7,-2,9),channel=c(2,1,2,1,1),cutoff=7),
 ties_promethion=list(time=c(100,100,100,0,7200),length=c(10,30,20,0,50),q=c(7,8,7,-2,9),channel=c(1002,1001,1002,1001,1001),cutoff=7),
 signed=list(time=c(7200,0,3599,3600),length=c(0,10,20,30),q=c(-3,-2,0,2),channel=c(1,2,1,2),cutoff=-2),
 zero=list(time=c(0,7200),length=c(0,0),q=c(0,7),channel=c(1,2),cutoff=7),
 sparse=list(time=c(1,7200000),length=c(1,1000000),q=c(7,20),channel=c(1,2),cutoff=7),
 singleton=list(time=0,length=10,q=7,channel=1,cutoff=7))
output <- list()
for(name in names(cases)) {
 c <- cases[[name]]
 env <- new.env(parent=globalenv())
 env$q <- c$cutoff; env$q_title <- paste0("Q>=",c$cutoff); env$muxes <- numeric()
 raw <- data.frame(start_time=c$time,sequence_length_template=c$length,mean_qscore_template=c$q,channel=c$channel)
 a <- add_cols(raw,-Inf); a$Q_cutoff <- "All reads"
 b <- add_cols(raw,c$cutoff); b$Q_cutoff <- env$q_title
 env$d <- rbind(a,b)
 # Actual single.flowcell expressions, including its missing-hour fill.
 for(i in which(labels=="f")[1]:which(labels=="p9")) eval(body[[i]],envir=env)
 eval(body[[which(labels=="p5")]],envir=env)
 hourly <- suppressWarnings(ggplot_build(env$p9))$data[[3]]
 cumulative <- suppressWarnings(ggplot_build(env$p5))$data[[2]]
 c$hourly_layer <- hourly[,c("x","y","group")]
 c$cumulative_layer <- cumulative[,c("x","y","group")]
 c$colour_labels <- ggplot_build(env$p5)$plot$scales$get_scales("colour")$get_labels()
 output[[name]] <- c
}
# Regression for the audit itself: a vertical tied jump must not be replaced
# by a diagonal to its upper endpoint during numerical comparison.
source("tests/reference/plot_layers.R")
temporal_candidates <- list(list(id="tie-test",observations=5,points=list(
 list(hours=0,yield_bp=0),list(hours=1,yield_bp=10),list(hours=1,yield_bp=100),list(hours=2,yield_bp=200))))
check <- temporal_comparisons(data.frame(x=c(0,.5,1,1,2),y=c(0,5,10,100,200)/1e9))[[1]]
stopifnot(check$maximum_envelope_error_bp==0,check$maximum_original_polyline_error_bp==0,check$unmatched_native_coordinates==0,check$maximum_coordinate_error_hours==0)
write_json(list(reference=list(commit=revision,r=as.character(getRversion()),ggplot2=as.character(packageVersion("ggplot2")),mgcv=as.character(packageVersion("mgcv"))),cases=output),args[2],digits=NA,auto_unbox=TRUE,pretty=TRUE)
