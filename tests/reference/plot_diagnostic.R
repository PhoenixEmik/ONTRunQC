# Actual numerical layer diagnostics, never pixel differences/substitute renderers.
args <- commandArgs(trailingOnly=TRUE)
d <- jsonlite::read_json(args[1],simplifyVector=TRUE)
png(args[2],width=720,height=500,type="cairo")
r <- d$reference;p <- d$native;res <- d$residual
has <- function(x) is.matrix(x)&&nrow(x)>0
if(isTRUE(d$yield_length) && length(d$measurements$yield_samples)) {
  s <- d$measurements$yield_samples
  par(mar=c(4,5,3,1),mfrow=c(2,1))
  plot(s$length_bp,s$reference_upper_bp/1e9,type="l",col="#94a3b8",xlab="Minimum length (bp)",ylab="Retained yield (Gb)",main="Actual R ordinates / inclusive truth / native display")
  segments(s$length_bp,s$reference_lower_bp/1e9,s$length_bp,s$reference_upper_bp/1e9,col="#475569")
  lines(s$length_bp,s$reference_first_bp/1e9,col="#475569")
  lines(s$length_bp,s$native_bp/1e9,col="#4f46e5",lty=2)
  abline(v=d$measurements$reference_axis_xmax_bp,col="#c2410c",lty=3)
  legend("topright",c("R tied ordinate range / first ordinate","Exact inclusive upper envelope","Native adaptive envelope","R X crop"),col=c("#475569","#94a3b8","#4f46e5","#c2410c"),lty=c(1,1,2,3),bty="n",cex=.7)
  plot(s$length_bp,s$residual_bp,type="l",col="#4f46e5",xlab="Minimum length (bp)",ylab="Native - inclusive truth (bp)",main=paste("Bounded samples; full maximum =",round(d$measurements$inclusive_display_error_bp,3),"bp"))
  abline(h=0,col="#94a3b8",lty=2)
  dev.off()
  quit(save="no")
}
separate <- isTRUE(d$histogram)&&has(r)&&has(p)
par(mar=c(4,5,2,1),mfrow=c(if(separate)3 else 2,1))
if(separate) {
  xr <- range(c(r[,1],p[,1]),finite=TRUE)
  plot(r,type="h",col="#475569",xlim=xr,xlab=d$x_label,ylab=d$y_label,main="Original R bin counts (own Y scale)")
  plot(p,type="h",col="#4f46e5",xlim=xr,xlab=d$x_label,ylab=d$y_label,main="Native bin counts (own Y scale); different boundaries")
} else if(has(r)&&has(p)) {
  plot(r,type="l",col="#475569",xlim=range(c(r[,1],p[,1])),ylim=range(c(r[,2],p[,2])),
       xlab=d$x_label,ylab=d$y_label,main="Actual values; see population/units in caption")
  lines(p,col="#4f46e5");legend("topright",c("Original R","ONTRunQC"),col=c("#475569","#4f46e5"),lty=1,bty="n",cex=.8)
} else if(has(p)) {
  plot(p,type="h",col="#4f46e5",xlab=d$x_label,ylab=d$y_label,main="Conserved native population (no false residual)")
} else {plot.new();text(.5,.5,"No commensurable finite curve",cex=1.2)}
if(has(res)) {
  plot(res,type="l",col="#c2410c",xlab=d$x_label,ylab=paste("Native - R:",d$y_label),main="Numerical difference, matched X only")
  abline(h=0,col="#94a3b8",lty=2)
} else {
  plot.new()
  text(0,1,paste(strwrap(d$description,width=100),collapse="\n"),adj=c(0,1),cex=.85)
  if(!separate) text(0,.3,paste("Reference built rows:",d$measurements$reference_layer_rows,
    "\nNative summary/basecalled:",d$measurements$native_total_records,"/",d$measurements$native_basecalled_reads),adj=c(0,1),cex=.85)
}
dev.off()
