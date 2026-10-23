# Optional isolated-backend investigation, never an original-reference target.
# r_backend.py gam_stability.R PAIRED_ROOT OUTPUT.json FROZEN_PENALTIES.rds
# The first backend creates the frozen file; later backends only read it.
suppressPackageStartupMessages(library(mgcv))
suppressPackageStartupMessages(library(jsonlite))
stopifnot(getRversion()=="4.4.1",packageVersion("ggplot2")=="3.5.1",packageVersion("mgcv")=="1.9-1")
args <- commandArgs(TRUE); stopifnot(length(args)==3)
probe <- Sys.getenv("PG_BACKEND_PROBE"); stopifnot(file.exists(probe))
dyn.load(probe)
sha <- function(p) strsplit(system2("shasum",c("-a","256",shQuote(p)),stdout=TRUE)," ")[[1]][1]
assigned <- function(e) if(is.call(e) && as.character(e[[1]]) %in% c("=","<-") && is.symbol(e[[2]])) as.character(e[[2]]) else ""
# Reuse common-nodal transforms from the unchanged reference capture/helpers.
for (file in c("tests/reference/capture.R","tests/reference/gam_assessment.R"))
  for(e in parse(file)) if(assigned(e) %in% c("gam_audit","model_values")) eval(e)
next_value <- function(x,n) {
  for(i in seq_len(abs(n))) x <- .C("ontrunqc_nextafter",as.double(x),as.integer(sign(n)))[[1]]
  x
}
raw_basis <- function(k) mgcv:::smooth.construct.cr.smooth.spec(s(x,bs="cr",k=10),list(x=k),list(x=k))
canonical <- function(k) {
  linear <- k-k[1]; linear <- linear/sqrt(sum(linear^2))
  constant <- 1-sum(linear)*linear
  cbind(linear,constant/sqrt(sum(constant^2)))
}
matrix_values <- function(S,k) {
  e <- eigen(S,symmetric=TRUE); z <- e$vectors[,9:10,drop=FALSE]
  b <- canonical(k)
  adjusted <- e$values; adjusted[9:10] <- e$values[8]*c(.1,.01)
  shrink <- e$vectors %*% (adjusted*t(e$vectors))
  list(raw_penalty=S,raw_symmetry_error=max(abs(S-t(S))),eigenvalues=e$values,
    numerical_null_rank=sum(abs(e$values)<=max(abs(e$values))*10*.Machine$double.eps),
    positive_null_gap=e$values[8]-max(abs(e$values[9:10])),
    null_projector=z%*%t(z),analytic_null_projector=b%*%t(b),
    null_residual=max(abs(S%*%z))/max(abs(S)),
    projector_error=max(abs(z%*%t(z)-b%*%t(b))),
    null_weight_in_geometric_basis=t(b)%*%shrink%*%b/e$values[8],
    shrink_penalty=shrink,normalized_shrink_penalty=shrink/max(e$values))
}
mode <- "canonical"; supplied <- NULL
smooth.construct.pgfinal.smooth.spec <- function(object,data,knots) {
  object <- mgcv:::smooth.construct.cr.smooth.spec(object,data,knots)
  e <- eigen(object$S[[1]],symmetric=TRUE)
  if(mode=="canonical") e$vectors[,9:10] <- canonical(object$xp)
  e$values[9:10] <- e$values[8]*if(mode=="swapped") c(.01,.1) else c(.1,.01)
  object$S[[1]] <- if(mode=="shared") supplied else e$vectors%*%(e$values*t(e$vectors))
  object$rank <- 10; object$null.space.dim <- 0; object
}
capture <- function(model,grid) {
  values <- model_values(model,grid)
  layer <- ggplot2:::predictdf(model,xseq=grid,se=TRUE,level=.95)
  values$uncensored_ggplot_prediction <- layer
  values$audit <- gam_audit(model,layer)
  values
}
fit <- function(d,k,bs="cs") gam(y~s(x,bs=bs,k=10),data=d,knots=list(x=k),method="REML")
cases <- list()
for(spec in list(c("minion","source_001"),c("promethion","source_001"),c("1d2","source_001"))) {
  case <- spec[1]; source <- spec[2]
  path <- file.path(args[1],case,"inputs",source,"sequencing_summary.txt")
  d <- read.delim(path,check.names=FALSE);d <- d[order(d$start_time),]
  two <- "sequence_length_2d" %in% names(d)
  lc <- if(two) "sequence_length_2d" else "sequence_length_template"
  qc <- if(two) "mean_qscore_2d" else "mean_qscore_template"
  cutoff <- if(case=="1d2") 8 else 7
  d <- d[d[[qc]]>=cutoff,]
  rust <- fromJSON(file.path(args[1],case,"source-population.json"),simplifyVector=FALSE)$reference_sources[[1]]
  reference <- fromJSON(file.path(args[1],case,"capture.json"),simplifyVector=FALSE)
  for(quality in c(FALSE,TRUE)) {
    audit <- Filter(function(a) a$quality==quality && startsWith(a$population,"qscore"),rust$gam_audit)[[1]]
    family <- if(quality) "q_by_hour" else "length_by_hour"
    plot <- Filter(function(p) p$directory==source && sub("\\.[^.]+$","",p$file)==family,reference$plots)[[1]]
    reference_knots <- unlist(plot$smooth_predictions[[2]]$knots)
    stopifnot(length(reference_knots)==10,max(abs(reference_knots-unlist(audit$basis$knots)))<1e-12)
    cases[[paste(case,if(quality) "quality" else "length",sep="_")]] <- list(
      data=data.frame(x=d$start_time/3600,y=d[[if(quality) qc else lc]]),knots=reference_knots,
      input_sha256=sha(path),rust_audit=audit)
  }
}
for(name in c("regular","irregular","repeated")) {
  i <- 0:1499; x <- if(name=="irregular") (i/1499)^3*40 else if(name=="repeated") floor(i/3)/499*40 else i/1499*40
  d <- data.frame(x=x,y=1000+30*x+100*sin(x)+sin(i*17)*50)
  cases[[name]] <- list(data=d,knots=quantile(unique(x),seq(0,1,length=10),names=FALSE))
}
frozen <- if(file.exists(args[3])) readRDS(args[3]) else list()
models <- list(); matrix_tests <- list()
for(name in names(cases)) {
  c <- cases[[name]];d <- c$data;k <- c$knots
  raw <- raw_basis(k)$S[[1]];mat <- matrix_values(raw,k)
  if(is.null(frozen[[name]])) frozen[[name]] <- list(raw=raw,shrink=mat$shrink_penalty,knots=k)
  stopifnot(identical(k,frozen[[name]]$knots))
  grid <- seq(min(d$x),max(d$x),length.out=80)
  base <- fit(d,k);repeated <- fit(d,k)
  baseline <- capture(base,grid); again <- capture(repeated,grid)
  variants <- list(original=baseline)
  for(m in c("swapped","canonical","shared")) {
    mode <- m; supplied <- frozen[[name]]$shrink
    variants[[m]] <- capture(fit(d,k,"pgfinal"),grid)
  }
  # Same original penalty bytes across backends: isolate optimizer/BLAS effects.
  mode <- "shared"; supplied <- frozen[[name]]$shrink
  reverse <- capture(fit(d[nrow(d):1,],k,"pgfinal"),grid)
  selected <- unique(round(seq(1,nrow(d),length.out=12)))
  models[[name]] <- list(observations=nrow(d),input_sha256=c$input_sha256,knots=k,hours=grid,
    matrix=mat,frozen_matrix=matrix_values(frozen[[name]]$raw,k),
    variants=variants,repeat_identical=identical(baseline,again),
    repeat_prediction_error=max(abs(baseline$fitted-again$fitted)),
    reverse_shared=reverse,
    design_sample=predict(base,newdata=d[selected,,drop=FALSE],type="lpmatrix"),
    rust_audit=c$rust_audit)
  # Full one-ULP/both-direction knot sweep on the affected public population
  # and independent synthetic shapes. No fixture-specific branch in the solver.
  if(name %in% c("minion_length","regular","irregular","repeated")) {
    for(j in 2:9) for(steps in c(-1024L,-4L,-1L,1L,4L,1024L)) {
      changed <- k; changed[j] <- next_value(k[j],steps)
      sm <- raw_basis(changed)$S[[1]];cm <- matrix_values(sm,changed)
      changed_model <- capture(fit(d,changed),grid)
      matrix_tests[[paste(name,j,steps,sep=":")]] <- list(case=name,knot_index=j,ulp_steps=steps,
        maximum_knot_delta=max(abs(changed-k)),matrix=cm,
        fit=changed_model)
    }
    for(scale in c(1e-9,1,1e9)) for(perturb in c(-1,0,1)) {
      S <- raw*scale
      S[2,3] <- S[3,2] <- S[2,3]+perturb*max(abs(S))*.Machine$double.eps
      matrix_tests[[paste(name,"matrix",scale,perturb,sep=":")]] <- list(case=name,
        scale=scale,off_diagonal_epsilon=perturb,matrix=matrix_values(S,k))
    }
  }
  cat(name,"completed\n")
}
if(!file.exists(args[3])) saveRDS(frozen,args[3],version=3)
# Force both BLAS and LAPACK paths to be mapped before recording the process.
invisible(eigen(diag(10)));invisible(crossprod(matrix(1:20,10)))
loaded <- paste0(args[2],".loaded.tsv")
invisible(.C("ontrunqc_backend_probe",loaded))
write_json(list(environment=list(r=as.character(getRversion()),ggplot2=as.character(packageVersion("ggplot2")),
  mgcv=as.character(packageVersion("mgcv")),blas=extSoftVersion()[["BLAS"]],lapack=La_library(),lapack_version=La_version(),
  session=capture.output(sessionInfo()),runtime=Sys.getenv(c("OPENBLAS_NUM_THREADS","OPENBLAS_CORETYPE","OMP_NUM_THREADS","DYLD_LIBRARY_PATH"))),
  frozen_sha256=sha(args[3]),driver_sha256=sha(sub("^--file=","",grep("^--file=",commandArgs(),value=TRUE))),
  loaded_images_file=loaded,models=models,perturbations=matrix_tests),args[2],auto_unbox=TRUE,digits=17,pretty=TRUE,null="null",na="null")
