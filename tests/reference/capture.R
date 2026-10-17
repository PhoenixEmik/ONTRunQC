# Execute the unmodified, hash-pinned MinIONQC script. The only interception is
# ggsave: call the real renderer and additionally retain numerical ggplot data.
suppressPackageStartupMessages(library(ggplot2))
suppressPackageStartupMessages(library(jsonlite))
suppressPackageStartupMessages(library(yaml))
stopifnot(getRversion() == "4.4.1", packageVersion("ggplot2") == "3.5.1",
          packageVersion("mgcv") == "1.9-1")
set.seed(1)
plots <- list()
problems <- character()
original_save <- ggplot2::ggsave
ggsave <- function(filename, plot, ...) {
  built <- ggplot2::ggplot_build(plot)
  layers <- lapply(seq_along(plot$layers), function(i) {
    layer <- built$plot$layers[[i]]
    method <- layer$computed_stat_params$method
    list(stat=class(layer$stat)[1],
         method=if (is.function(method)) "gam" else method,
         formula=paste(deparse(layer$computed_stat_params$formula), collapse=" "),
         data=as.data.frame(built$data[[i]]))
  })
  colour <- built$plot$scales$get_scales("colour")
  labels <- if (is.null(colour)) NULL else colour$get_labels()
  plots[[length(plots)+1L]] <<- list(file=basename(filename),
      directory=basename(dirname(filename)), labels=lapply(plot$labels, function(x) paste(as.character(x),collapse=" ")),
      colour_labels=labels, colours=if(is.null(colour)) NULL else colour$map(labels),
      panels=as.data.frame(built$layout$layout), layers=layers)
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
write_json(list(error=error,warnings=problems,session=capture.output(sessionInfo()),
  packages=as.data.frame(installed.packages()[,c("Package","Version")]),
  summaries=summary_values,populations=populations,channels=channels,plots=plots),
  Sys.getenv("PG_REFERENCE_CAPTURE"),auto_unbox=TRUE,pretty=TRUE,digits=17,na="null",null="null")
if (!is.null(error)) quit(status=1)
