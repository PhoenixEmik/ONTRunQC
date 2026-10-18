# Execute the unmodified, hash-pinned MinIONQC script. The only interception is
# ggsave: call the real renderer and additionally retain numerical ggplot data.
suppressPackageStartupMessages(library(ggplot2))
suppressPackageStartupMessages(library(jsonlite))
suppressPackageStartupMessages(library(yaml))
environment <- list(r=as.character(getRversion()),
  ggplot2=as.character(packageVersion("ggplot2")),
  mgcv=as.character(packageVersion("mgcv")),
  session=capture.output(sessionInfo()),
  packages=as.data.frame(installed.packages()[,c("Package","Version")]))
if (Sys.getenv("PG_REFERENCE_PREFLIGHT") == "true") {
  write_json(environment, Sys.getenv("PG_REFERENCE_CAPTURE"),
    auto_unbox=TRUE, pretty=TRUE)
  quit(status=if (environment$r == "4.4.1" && environment$ggplot2 == "3.5.1" &&
    environment$mgcv == "1.9.1") 0 else 2)
}
stopifnot(getRversion() == "4.4.1", packageVersion("ggplot2") == "3.5.1",
          packageVersion("mgcv") == "1.9-1")
set.seed(1)
plots <- list()
problems <- character()
density_calls <- list()
smooth_calls <- list()
audit_errors <- character()
gam_audit <- function(model, prediction) {
  knots <- unname(model$smooth[[1]]$xp)
  raw <- mgcv::smoothCon(mgcv::s(x, bs="cr", k=10),
    data=data.frame(x=knots), knots=list(x=knots), absorb.cons=FALSE,
    scale.penalty=FALSE)[[1]]
  shrink <- mgcv::smoothCon(mgcv::s(x, bs="cs", k=10),
    data=data.frame(x=knots), knots=list(x=knots), absorb.cons=FALSE,
    scale.penalty=FALSE)[[1]]
  transform <- predict(model, newdata=data.frame(x=knots), type="lpmatrix")
  inverse <- solve(transform)
  penalty <- matrix(0,10,10)
  penalty[2:10,2:10] <- model$smooth[[1]]$S[[1]]
  eig <- eigen(raw$S[[1]], symmetric=TRUE)
  list(raw_penalty=raw$S[[1]], second_derivatives=t(matrix(raw$F,10,10)),
    shrink_penalty=shrink$S[[1]],
    raw_eigenvalues=eig$values, raw_eigenvectors=eig$vectors,
    model_transform=unname(transform), model_penalty=penalty,
    model_crossproduct=crossprod(model$R),
    basis_at_predictions=unname(mgcv::PredictMat(raw,data.frame(x=prediction$x))),
    coefficients=unname(model$coefficients), covariance=unname(model$Vp),
    nodal_coefficients=unname(transform %*% model$coefficients),
    nodal_covariance=unname(transform %*% model$Vp %*% t(transform)),
    effective_nodal_penalty=t(inverse) %*% penalty %*% inverse,
    penalty_scale=model$smooth[[1]]$S.scale,
    reml=unname(model$gcv.ubre), edf=sum(model$edf),
    residual_sum_squares=model$deviance,
    optimizer=model$outer.info)
}
record_smooth <- function(model, prediction) {
  if (is.null(prediction)) return(invisible(NULL))
  smooth_calls[[length(smooth_calls)+1L]] <<- list(
    method=if(inherits(model,"gam")) "gam" else "loess",
    knots=if(inherits(model,"gam")) model$smooth[[1]]$xp else NULL,
    smoothing_parameter=if(inherits(model,"gam")) unname(model$sp) else NULL,
    residual_variance=if(inherits(model,"gam")) model$sig2 else model$s^2,
    audit=if(inherits(model,"gam")) tryCatch(gam_audit(model,prediction),
      error=function(e) { audit_errors <<- c(audit_errors,conditionMessage(e)); NULL }) else NULL,
    observations=if(inherits(model,"gam")) length(model$y) else model$n,
    data=prediction)
}
invisible(trace("predictdf", where=asNamespace("ggplot2"), print=FALSE,
  exit=quote(.GlobalEnv$record_smooth(model, returnValue()))))
record_density <- function(bw, x, from, to, adjust, kernel, n) {
  density_calls[[length(density_calls)+1L]] <<- list(
    bandwidth=if (is.numeric(bw)) bw * adjust else NULL,
    observations=length(x), from=from, to=to, kernel=kernel, grid_points=n)
}
# Observe the actual bandwidth/domain inside ggplot2, without substituting an
# independent estimator or changing any returned statistical values.
invisible(trace("compute_density", where=asNamespace("ggplot2"), print=FALSE,
  exit=quote(.GlobalEnv$record_density(bw, x, from, to, adjust, kernel, n))))
original_save <- ggplot2::ggsave
ggsave <- function(filename, plot, ...) {
  density_calls <<- list()
  smooth_calls <<- list()
  built <- ggplot2::ggplot_build(plot)
  layers <- lapply(seq_along(plot$layers), function(i) {
    layer <- built$plot$layers[[i]]
    method <- layer$computed_stat_params$method
    list(stat=class(layer$stat)[1],
         method=if (is.function(method)) "gam" else method,
         formula=paste(deparse(layer$computed_stat_params$formula), collapse=" "),
         data=if(class(layer$stat)[1] %in% c("StatSmooth","StatDensity")) as.data.frame(built$data[[i]]) else NULL,
         rows=nrow(built$data[[i]]), groups=unique(built$data[[i]]$group))
  })
  colour <- built$plot$scales$get_scales("colour")
  breaks <- if (is.null(colour)) NULL else colour$get_breaks()
  labels <- if (is.null(colour)) NULL else colour$get_labels(breaks)
  plots[[length(plots)+1L]] <<- list(file=basename(filename),
      directory=basename(dirname(filename)), labels=lapply(plot$labels, function(x) paste(as.character(x),collapse=" ")),
      colour_labels=labels, colours=if(is.null(colour)) NULL else colour$map(breaks),
      panels=as.data.frame(built$layout$layout), layers=layers,
      density_parameters=density_calls, smooth_predictions=smooth_calls)
  if (Sys.getenv("PG_REFERENCE_RENDER", "true") == "true")
    original_save(filename=filename, plot=plot, ...)
  invisible(filename)
}
error <- NULL
withCallingHandlers(tryCatch(source(Sys.getenv("PG_REFERENCE_SOURCE"), local=globalenv()),
  error=function(e) error <<- conditionMessage(e)), warning=function(w) {
  problems <<- c(problems, conditionMessage(w)); invokeRestart("muffleWarning")
})
# The actual returned MinIONQC populations, before any Python reconstruction.
dataset <- if (exists("dm", inherits=FALSE)) dm else if (exists("d", inherits=FALSE)) d else NULL
populations <- list()
channels <- list()
if (!is.null(dataset)) for (label in unique(as.character(dataset$Q_cutoff))) {
  rows <- dataset[as.character(dataset$Q_cutoff)==label,]
  populations[[label]] <- list(reads=nrow(rows), bases=sum(rows$sequence_length_template),
    zero_lengths=sum(rows$sequence_length_template==0),
    mean_length=mean(rows$sequence_length_template), median_length=median(rows$sequence_length_template),
    mean_q=mean(rows$mean_qscore_template), median_q=median(rows$mean_qscore_template))
  # Preserve the original channel.summary calculation, including its count bug.
  channels[[label]] <- channel.summary(rows)
}
summaries <- list.files(opt$output.dir, pattern="^summary.yaml$", full.names=TRUE, recursive=TRUE)
summary_values <- lapply(summaries, function(path) list(directory=basename(dirname(path)), data=yaml::read_yaml(path)))
if (length(audit_errors)) error <- paste("Reference capture audit failed:",paste(audit_errors,collapse="; "))
write_json(list(error=error,warnings=problems,session=capture.output(sessionInfo()),
  packages=as.data.frame(installed.packages()[,c("Package","Version")]),
  summaries=summary_values,populations=populations,channels=channels,plots=plots),
  Sys.getenv("PG_REFERENCE_CAPTURE"),auto_unbox=TRUE,pretty=TRUE,digits=17,na="null",null="null")
if (!is.null(error)) quit(status=1)
