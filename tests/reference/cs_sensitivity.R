# Optional numerical diagnostic, not a production analysis or altered reference.
# Reproduce the pinned mgcv cs null-space discontinuity on a public template TSV:
# Rscript tests/reference/cs_sensitivity.R INPUT.tsv 7 OUTPUT.json
suppressPackageStartupMessages(library(mgcv))
suppressPackageStartupMessages(library(jsonlite))
stopifnot(getRversion()=="4.4.1",packageVersion("mgcv")=="1.9-1")
args <- commandArgs(trailingOnly=TRUE)
stopifnot(length(args)==3)
cutoff <- as.numeric(args[2])
stopifnot(is.finite(cutoff),cutoff>=0)
d <- read.delim(args[1],check.names=FALSE)
d <- d[order(d$start_time),]
d <- d[d$mean_qscore_template>=cutoff,]
data <- data.frame(x=d$start_time/3600,y=d$sequence_length_template)
knots <- quantile(unique(data$x),seq(0,1,length=10),names=FALSE)
construct <- function(k) {
  sm <- mgcv:::smooth.construct.cr.smooth.spec(s(x,bs="cr",k=10),
    list(x=k),list(x=k))
  eig <- eigen(sm$S[[1]],symmetric=TRUE)
  list(penalty=sm$S[[1]],linear_high=abs(eig$vectors[1,9])<1e-8)
}
endpoint <- function(k) {
  model <- gam(y~s(x,bs="cs",k=10),data=data,knots=list(x=k),method="REML")
  as.numeric(predict(model,newdata=data.frame(x=max(data$x))))
}
base <- construct(knots)
prediction <- endpoint(knots)
results <- list()
for (i in 2:9) for (multiplier in c(-1,1,-2,2,-4,4)) {
  if (length(results)>=2) next
  changed <- knots
  changed[i] <- changed[i]+multiplier*.Machine$double.eps*abs(changed[i])
  model <- construct(changed)
  if (model$linear_high!=base$linear_high) {
    estimate <- endpoint(changed)
    results[[length(results)+1]] <- list(knot_index=i,
      epsilon_multiplier=multiplier,
      maximum_knot_difference_hours=max(abs(changed-knots)),
      maximum_raw_penalty_difference=max(abs(model$penalty-base$penalty)),
      linear_high=model$linear_high,prediction=estimate,
      prediction_difference=estimate-prediction)
  }
}
write_json(list(r=as.character(getRversion()),mgcv=as.character(packageVersion("mgcv")),
  observations=nrow(data),cutoff=cutoff,baseline_prediction=prediction,
  baseline_linear_high=base$linear_high,perturbations=results),
  args[3],auto_unbox=TRUE,digits=17,pretty=TRUE)
