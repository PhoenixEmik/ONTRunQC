# Optional synthetic-only golden capture; normal builds never require R.
# Rscript tests/reference/loess_reference.R OUTPUT.json
suppressPackageStartupMessages(library(ggplot2))
suppressPackageStartupMessages(library(jsonlite))
stopifnot(getRversion()=="4.4.1",packageVersion("ggplot2")=="3.5.1")
args <- commandArgs(trailingOnly=TRUE)
stopifnot(length(args)==1)
specs <- list(
  list(name="irregular",times=c(0,11,52,87,90,120,305,701,902,910,1010,2110)),
  list(name="small",times=(0:98)*71),
  list(name="near_method_boundary",times=(0:998)*31),
  list(name="tied",times=rep(c(0,71,200,800,1000,2500),each=4)),
  list(name="three",times=c(0,71,142)),
  list(name="four",times=c(0,71,142,213)),
  list(name="two",times=c(0,71)),
  list(name="constant_time",times=rep(71,12)))
cases <- list()
for(spec in specs) {
  i <- seq_along(spec$times)-1
  lengths <- as.integer(1000+200*sin(i*.7)+i*2)
  qs <- 7+2*cos(i*.4)
  lines <- c("channel\tstart_time\tsequence_length_template\tmean_qscore_template",
    paste(1,spec$times,lengths,format(qs,digits=17,trim=TRUE),sep="\t"))
  fits <- list()
  for(selected in c(FALSE,TRUE)) for(quality in c(FALSE,TRUE)) {
    keep <- !selected | qs>=7
    data <- data.frame(x=spec$times[keep]/3600,y=if(quality)qs[keep] else lengths[keep])
    data <- data[order(data$x),]
    warnings <- character()
    error <- NULL
    value <- withCallingHandlers(tryCatch({
      if(nrow(data)<3 || min(data$x)==max(data$x)) stop("insufficient observations or distinct times")
      model <- loess(y~x,data=data)
      points <- ggplot2:::predictdf(model,seq(min(data$x),max(data$x),length=80),TRUE,.95)
      list(trace_hat=model$trace.hat,one_delta=model$one.delta,two_delta=model$two.delta,
        residual_scale=model$s,df=model$one.delta^2/model$two.delta,
        critical_value=qt(.975,model$one.delta^2/model$two.delta),points=points)
    },error=function(e){ error <<- conditionMessage(e); NULL }),
      warning=function(w){ warnings <<- c(warnings,conditionMessage(w));invokeRestart("muffleWarning") })
    fits[[length(fits)+1]] <- list(selected=selected,quality=quality,error=error,
      warnings=unique(warnings),reference=value)
  }
  cases[[length(cases)+1]] <- list(name=spec$name,input=paste0(paste(lines,collapse="\n"),"\n"),fits=fits)
}
write_json(list(r=as.character(getRversion()),ggplot2=as.character(packageVersion("ggplot2")),
  session=capture.output(sessionInfo()),cases=cases),args[1],auto_unbox=TRUE,digits=17,
  pretty=FALSE,na="null",null="null")
