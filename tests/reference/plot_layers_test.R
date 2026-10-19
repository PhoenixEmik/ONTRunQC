# Optional real ggplot tests. No MinIONQC source or public fixture needed.
suppressPackageStartupMessages(library(ggplot2))
args <- commandArgs(trailingOnly=TRUE)
source(if(length(args)) args[1] else "tests/reference/plot_layers.R")
check <- function(plot,i=1L,family="") {
  built <- ggplot_build(plot)
  audit_layer(built$plot$layers[[i]],built$data[[i]],family=family)
}
d <- data.frame(x=c(0,1,1,2,3,10),y=c(0,1,2,4,8,16))
h <- check(ggplot(d,aes(x))+geom_histogram(bins=4))
stopifnot(h$retention=="complete_bounded_layer",sum(h$data$count)==6,
          all(h$data$xmin<h$data$xmax))
b <- check(ggplot(d,aes(x,y))+geom_bin2d(bins=3))
stopifnot(sum(b$data$count)==6)
c <- check(ggplot(d,aes(factor(x)))+geom_bar())
stopifnot(sum(c$data$count)==6)
large <- data.frame(x=rep(0:99,500),y=rep(c(1:499,Inf),100))
p <- check(ggplot(large,aes(x,y))+geom_point())
stopifnot(p$retention=="bounded_summary",is.null(p$data),nrow(p$sample)<=256,
  p$rows==50000,p$nonfinite_xy==100,sum(p$grid$counts)==49900)
y <- check(ggplot(d,aes(x,y))+geom_line(),family="yield_over_time.png")
q <- y$cumulative_queries[[1]]$data
stopifnot(q$y[q$x==1]==2,q$y[q$x==0]==0,q$y[q$x==10]==16)
cat("Histogram/bin2d/count conservation, finite-grid conservation, bounded retention and tied cumulative queries verified.\n")
