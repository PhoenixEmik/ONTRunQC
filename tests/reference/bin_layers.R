# Synthetic golden layers from the pinned ggplot2 engine. No private records.
library(ggplot2)
stopifnot(as.character(getRversion())=="4.4.1",as.character(packageVersion("ggplot2"))=="3.5.1",as.character(packageVersion("mgcv"))=="1.9.1")
args <- commandArgs(TRUE)
cases <- list(
 zero_axis=list(length=c(0,1,100),q=c(-100,10,20),cutoff=7),
 boundary=list(length=c(0,1,2,4,8,10,20,100,1000,1000000),q=c(0,-2,0,2,4,7,7,10,20,50),cutoff=7),
 constant_length=list(length=rep(100,5),q=c(-1,0,7,7,20),cutoff=7),
 constant_q=list(length=c(0,1,10,100,10000),q=rep(7,5),cutoff=7),
 zero=list(length=c(0,0),q=c(-5,12),cutoff=7),
 sparse=list(length=c(1,1000000000),q=c(-20,1000),cutoff=7),
 ties=list(length=c(1,1,10,10,100,100),q=c(0,0,1,1,2,2),cutoff=1),
 singleton=list(length=100,q=7,cutoff=7),
 empty=list(length=numeric(),q=numeric(),cutoff=7))
errors <- list()
extract <- function(p,cols) {
 b <- tryCatch(suppressWarnings(suppressMessages(ggplot_build(p))), error=function(e) {errors[[length(errors)+1L]] <<- list(case=name,message=conditionMessage(e));NULL})
 if(is.null(b)) return(list())
 if(!nrow(b$data[[1]])) return(list())
 d <- b$data[[1]]; d$population <- ifelse(as.integer(d$PANEL)==1,"all","q_selected")
 d[,intersect(c(cols,"population"),names(d)),drop=FALSE]
}
for(name in names(cases)) {
 c <- cases[[name]]; d <- data.frame(length=c$length,q=c$q)
 both <- rbind(transform(d,population=rep("All",nrow(d))),transform(subset(d,q>=c$cutoff),population=rep("Q",sum(d$q>=c$cutoff))))
 cases[[name]]$length_bins <- extract(ggplot(both,aes(length))+geom_histogram(bins=300)+scale_x_log10()+facet_wrap(~population,scales="free_y"),c("x","xmin","xmax","count"))
 cases[[name]]$q_bins <- extract(ggplot(both,aes(q))+geom_histogram(bins=300)+facet_wrap(~population,scales="free_y"),c("x","xmin","xmax","count"))
 xy <- extract(ggplot(d,aes(length,q))+geom_bin2d()+scale_x_log10(),c("x","xmin","xmax","y","ymin","ymax","count"))
 if(length(xy)) xy$population <- NULL
 cases[[name]]$cells <- xy
}
# The read-count family deliberately uses true n, rather than upstream's
# sum(which(...)) defect. All other formula inputs match channel.summary.
channel_values <- list(all=list(
 list(read_count=3,total_yield_bp=110,mean_read_length_bp=110/3,median_read_length_bp=10),
 list(read_count=2,total_yield_bp=0,mean_read_length_bp=0,median_read_length_bp=0),
 list(read_count=1,total_yield_bp=1000,mean_read_length_bp=1000,median_read_length_bp=1000)),
 q_selected=list(
 list(read_count=2,total_yield_bp=100,mean_read_length_bp=50,median_read_length_bp=50),
 list(read_count=1,total_yield_bp=0,mean_read_length_bp=0,median_read_length_bp=0),
 list(read_count=0,total_yield_bp=0,mean_read_length_bp=NULL,median_read_length_bp=NULL)))
channel_bins <- list()
for(metric in names(channel_values$all[[1]])) {
 a <- vapply(channel_values$all,function(v)v[[metric]],numeric(1))
 q <- vapply(channel_values$q_selected[1:2],function(v)v[[metric]],numeric(1))
 data <- data.frame(value=c(a,q),population=c(rep("All",length(a)),rep("Q",length(q))))
 channel_bins[[metric]] <- extract(ggplot(data,aes(value))+geom_histogram(bins=30)+facet_wrap(~population,scales="free_y"),c("x","xmin","xmax","count"))
}
jsonlite::write_json(list(reference=list(r="4.4.1",ggplot2="3.5.1",mgcv="1.9-1",minionqc="54485d586038d8ee36f168dd94f31c423f52f581"),cases=cases,channel_values=channel_values,channel_bins=channel_bins,upstream_errors=errors),args[1],digits=NA,auto_unbox=TRUE)
