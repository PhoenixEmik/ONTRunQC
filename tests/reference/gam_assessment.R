# Validation-only controlled models. Never substitute these for the unmodified
# reference comparisons. Use an existing pinned R installation:
# Rscript tests/reference/gam_assessment.R PAIRED_DIRECTORY OUTPUT.json
suppressPackageStartupMessages(library(mgcv))
suppressPackageStartupMessages(library(jsonlite))
stopifnot(getRversion()=="4.4.1",packageVersion("mgcv")=="1.9-1",
          packageVersion("ggplot2")=="3.5.1")
args <- commandArgs(trailingOnly=TRUE)
stopifnot(length(args) %in% c(2,3))
root <- normalizePath(args[1])
mode <- "canonical"
rust_penalty <- NULL
# All variants share the same natural-cubic design. Only the diagnostic penalty
# is changed. The fitted model's internal centering/scaling is still mgcv's own.
smooth.construct.pgdiagnostic.smooth.spec <- function(object,data,knots) {
  object <- mgcv:::smooth.construct.cr.smooth.spec(object,data,knots)
  e <- eigen(object$S[[1]],symmetric=TRUE)
  if (mode=="canonical") {
    linear <- object$xp-object$xp[1]
    linear <- linear/sqrt(sum(linear^2))
    constant <- 1-sum(linear)*linear
    e$vectors[,9] <- linear
    e$vectors[,10] <- constant/sqrt(sum(constant^2))
  }
  e$values[9:10] <- e$values[8]*if(mode=="swapped") c(.01,.1) else c(.1,.01)
  object$S[[1]] <- if(mode=="rust") rust_penalty else
    e$vectors %*% (e$values*t(e$vectors))
  object$rank <- 10
  object$null.space.dim <- 0
  object
}
failures <- function(r,p) sum(abs(r-p)>1e-6+1e-6*abs(r))
maximum <- function(r,p) max(abs(r-p))
model_values <- function(model,x,supplied_sp=NULL) {
  p <- predict(model,newdata=data.frame(x=x),se.fit=TRUE)
  transform <- predict(model,newdata=data.frame(x=model$smooth[[1]]$xp),type="lpmatrix")
  list(sp=if(length(model$sp)) unname(model$sp) else supplied_sp,
    smoothing_parameter_fixed=!is.null(supplied_sp),penalty_scale=model$smooth[[1]]$S.scale,
    reml=unname(model$gcv.ubre),edf=sum(model$edf),variance=model$sig2,
    optimized_log_parameters=unname(model$outer.info$lsp),
    convergence=model$outer.info$conv,iterations=model$outer.info$iter,
    fitted=unname(p$fit),standard_error=unname(p$se.fit),
    lower=unname(p$fit-1.959963984540054*p$se.fit),
    upper=unname(p$fit+1.959963984540054*p$se.fit),
    nodal_coefficients=unname(transform%*%model$coefficients),
    nodal_covariance=unname(transform%*%model$Vp%*%t(transform)))
}
results <- list()
save_results <- function() {
  sha256 <- function(path) strsplit(system2("shasum",c("-a","256",shQuote(path)),stdout=TRUE)," ")[[1]][1]
  script <- sub("^--file=","",grep("^--file=",commandArgs(),value=TRUE))
  reference <- fromJSON(file.path(root,"comparison.json"),simplifyVector=FALSE)
  write_json(list(environment=list(r=as.character(getRversion()),
    ggplot2=as.character(packageVersion("ggplot2")),mgcv=as.character(packageVersion("mgcv")),
    session=capture.output(sessionInfo()),blas=extSoftVersion()[["BLAS"]],lapack=La_library()),
    source_commit=reference$reference_commit,reference_source_sha256=reference$source_sha256,
    driver_sha256=sha256(script),models=results),args[2],auto_unbox=TRUE,digits=17,pretty=TRUE,null="null",na="null")
}
specs <- list(c("minion","source_001"),c("promethion","source_001"),
  c("1d2","source_001"),c("combined","source_002"),c("fit-large","source_001"))
for (spec in specs) {
  case <- spec[1]; source <- spec[2]
  if(length(args)==3 && case!=args[3]) next
  path <- file.path(root,case,"inputs",source,"sequencing_summary.txt")
  d <- read.delim(path,check.names=FALSE)
  d <- d[order(d$start_time),]
  two_d <- "sequence_length_2d" %in% names(d)
  length_column <- if(two_d) "sequence_length_2d" else "sequence_length_template"
  q_column <- if(two_d) "mean_qscore_2d" else "mean_qscore_template"
  if("calibration_strand_genome_template" %in% names(d))
    d <- d[d$calibration_strand_genome_template!="YHR174W",]
  cutoff <- if(case=="1d2") 8 else 7
  probe <- fromJSON(file.path(root,case,"source-population.json"),simplifyVector=FALSE)
  audit <- Filter(function(s)s$source_id==source,probe$reference_sources)[[1]]$gam_audit
  for(selected in c(FALSE,TRUE)) for(quality in c(FALSE,TRUE)) {
    rows <- if(selected) d[d[[q_column]]>=cutoff,] else d
    positive_q_single <- case!="combined" && quality && !selected
    if(positive_q_single) rows <- rows[rows[[q_column]]>=0,]
    data <- data.frame(x=rows$start_time/3600,y=rows[[if(quality) q_column else length_column]])
    rust <- Filter(function(a) a$quality==quality &&
      startsWith(a$population,"qscore")==selected && a$single==positive_q_single,audit)[[1]]
    # For public data, the positive-Q single and all-record combined populations
    # are identical; the normal harness still tests the distinct selection rule.
    k <- unlist(rust$basis$knots)
    grid <- min(data$x)+(max(data$x)-min(data$x))*(0:79)/79
    base <- gam(y~s(x,bs="cs",k=10),data=data,knots=list(x=k),method="REML")
    repeated <- gam(y~s(x,bs="cs",k=10),data=data,knots=list(x=k),method="REML")
    baseline <- model_values(base,grid)
    repeat_values <- model_values(repeated,grid)
    mode <- "swapped"
    swapped <- gam(y~s(x,bs="pgdiagnostic",k=10),data=data,knots=list(x=k),method="REML")
    mode <- "canonical"
    canonical <- gam(y~s(x,bs="pgdiagnostic",k=10),data=data,knots=list(x=k),method="REML")
    raw <- mgcv:::smooth.construct.cr.smooth.spec(s(x,bs="cr",k=10),list(x=k),list(x=k))
    largest <- max(eigen(raw$S[[1]],symmetric=TRUE,only.values=TRUE)$values)
    rust_penalty <- do.call(rbind,lapply(rust$basis$normalized_shrink_penalty,unlist))*largest
    mode <- "rust"
    identical_penalty <- gam(y~s(x,bs="pgdiagnostic",k=10),data=data,knots=list(x=k),method="REML")
    lambda <- exp(rust$fit$log_lambda)
    fixed <- gam(y~s(x,bs="pgdiagnostic",k=10),data=data,knots=list(x=k),method="REML",
      sp=lambda*identical_penalty$smooth[[1]]$S.scale/largest)
    context <- if(case=="combined") "combined" else "single"
    curves <- Filter(function(s)s$source_id==source,probe$reference_sources)[[1]]$fitted_trends[[context]]
    curve <- Filter(function(c)(c$metric=="qscore")==quality &&
      startsWith(c$population,"qscore")==selected,curves)[[1]]
    rust_points <- lapply(c("fitted","standard_error","lower","upper"),function(field)
      vapply(curve$points,function(p)p[[if(field=="fitted") "value" else field]],numeric(1)))
    names(rust_points) <- c("fitted","standard_error","lower","upper")
    variants <- list(baseline=baseline,swapped=model_values(swapped,grid),
      canonical=model_values(canonical,grid),identical_penalty=model_values(identical_penalty,grid),
      fixed_lambda=model_values(fixed,grid,lambda*identical_penalty$smooth[[1]]$S.scale/largest))
    errors <- lapply(variants,function(v) {
      fields <- lapply(names(rust_points),function(f) list(maximum_error=maximum(v[[f]],rust_points[[f]]),
        failed_points=failures(v[[f]],rust_points[[f]])))
      names(fields) <- names(rust_points)
      c(fields,list(normalized_lambda=v$sp*largest/v$penalty_scale,
        log_lambda_difference=log(v$sp*largest/v$penalty_scale)-rust$fit$log_lambda,
        residual_variance_difference=v$variance-rust$fit$residual_variance,
        nodal_coefficients_maximum_error=maximum(v$nodal_coefficients,unlist(rust$fit$nodal_coefficients)),
        nodal_coefficient_failures=failures(v$nodal_coefficients,unlist(rust$fit$nodal_coefficients)),
        nodal_covariance_maximum_error=maximum(v$nodal_covariance,
          do.call(rbind,lapply(rust$fit$nodal_covariance,unlist))),
        nodal_covariance_failures=failures(v$nodal_covariance,
          do.call(rbind,lapply(rust$fit$nodal_covariance,unlist)))))
    })
    results[[length(results)+1]] <- list(case=case,source=source,selected=selected,
      input_sha256=strsplit(system2("shasum",c("-a","256",shQuote(path)),stdout=TRUE)," ")[[1]][1],
      quality=quality,observations=nrow(data),knots=k,hours=grid,
      repeat_maximum_error=maximum(baseline$fitted,repeat_values$fitted),
      repeat_identical=identical(baseline,repeat_values),variants=variants,errors=errors,
      rust=list(log_lambda=rust$fit$log_lambda,variance=rust$fit$residual_variance,
        reml=rust$fit$reml,edf=rust$fit$edf,points=rust_points))
    save_results()
    cat(case,selected,quality,"baseline",errors$baseline$fitted$maximum_error,
        "canonical",errors$canonical$fitted$maximum_error,
        "fixed",errors$fixed_lambda$fitted$maximum_error,"\n")
  }
}
save_results()
