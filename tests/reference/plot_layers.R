# Optional, bounded observation of ALREADY built ggplot layers. No recomputation
# or modification of plot inputs/statistical results. Coordinates are post-scale,
# pre-coordinate transformation (e.g. histogram length boundaries are log10 bp).
audit_scale <- function(scale) list(
  transformation=scale$trans$name, limits=scale$get_limits(),
  trained_range=scale$range$range, breaks=scale$get_breaks())

# Only observe the original already-computed layer. Native curves are supplied
# as queries; they never replace the R input, statistical result or renderer.
temporal_candidates <- if(nzchar(Sys.getenv("PG_TEMPORAL_CANDIDATES")))
  jsonlite::fromJSON(Sys.getenv("PG_TEMPORAL_CANDIDATES"),simplifyVector=FALSE) else list()
yield_candidates <- if(nzchar(Sys.getenv("PG_YIELD_CANDIDATES")))
  jsonlite::fromJSON(Sys.getenv("PG_YIELD_CANDIDATES"),simplifyVector=FALSE) else list()

yield_comparisons <- function(d) {
  # Observe actual built GeomLine coordinates AFTER R's axis censoring. Preserve
  # tied row ordering; max/min are an explicitly separate cumulative envelope.
  d <- d[order(d$x,method="radix"),,drop=FALSE]
  first <- d[!duplicated(d$x),,drop=FALSE]
  last <- d[!duplicated(d$x,fromLast=TRUE),,drop=FALSE]
  keys <- match(d$x,first$x)
  upper <- as.numeric(tapply(round(d$y*1e9),keys,max))
  lower <- as.numeric(tapply(round(d$y*1e9),keys,min))
  # tapply orders numeric factor levels numerically (1..N), not X text values.
  lapply(yield_candidates,function(candidate) {
    p <- candidate$points
    if(!length(p)) return(list(candidate=candidate$id,unavailable=TRUE))
    x <- vapply(p,function(v)v$minimum_length_bp,numeric(1))
    y <- vapply(p,function(v)v$retained_yield_bp,numeric(1))
    head_y <- y[!duplicated(x)]; tail_y <- y[!duplicated(x,fromLast=TRUE)]
    x <- unique(x)
    index <- pmax(1L,pmin(length(x),findInterval(first$x,x)))
    next_index <- pmin(length(x),index+1L)
    same <- x[index]==first$x
    weight <- ifelse(x[index]==x[next_index],0,(first$x-x[index])/(x[next_index]-x[index]))
    interpolated <- tail_y[index]+weight*(head_y[next_index]-tail_y[index])
    inclusive <- ifelse(same,head_y[index],interpolated)
    right <- ifelse(same,tail_y[index],interpolated)
    truth <- candidate$exact_steps
    truth_x <- vapply(truth,function(v)v$length_bp,numeric(1))
    truth_y <- vapply(truth,function(v)v$inclusive_yield_bp,numeric(1))
    truth_n <- vapply(truth,function(v)v$read_count,numeric(1))
    match_index <- match(first$x,truth_x)
    matched <- !is.na(match_index)
    expected <- truth_y[match_index[matched]]
    expected_lower <- expected-(truth_n[match_index[matched]]-1)*first$x[matched]
    sample <- unique(round(seq(1,nrow(first),length.out=min(256L,nrow(first)))))
    list(candidate=candidate$id,reference_finite_observations=nrow(d),
      reference_unique_lengths=nrow(first),reference_tied_observations=sum(duplicated(d$x)),
      reference_xmin_bp=min(d$x),reference_xmax_bp=max(d$x),
      unmatched_reference_lengths=sum(!matched),
      maximum_reference_envelope_error_bp=if(any(matched)) max(abs(upper[matched]-expected)) else NULL,
      maximum_reference_lower_error_bp=if(any(matched)) max(abs(lower[matched]-expected_lower)) else NULL,
      reference_terminal_yield_bp=max(upper),native_terminal_yield_bp=head(y,1),
      maximum_inclusive_envelope_error_bp=max(abs(inclusive-upper)),
      maximum_original_ordinate_difference_bp=max(abs(inclusive-upper),abs(inclusive-lower)),
      maximum_original_one_sided_difference_bp=max(abs(inclusive-round(first$y*1e9)),abs(right-round(last$y*1e9))),
      maximum_tied_vertical_range_bp=max(upper-lower),
      samples=data.frame(length_bp=first$x[sample],reference_upper_bp=upper[sample],reference_lower_bp=lower[sample],
        reference_first_bp=round(first$y[sample]*1e9),reference_last_bp=round(last$y[sample]*1e9),
        native_bp=inclusive[sample],residual_bp=inclusive[sample]-upper[sample]))
  })
}
temporal_comparisons <- function(d) {
  original <- d[order(d$x,method="radix"),,drop=FALSE]
  first <- original[!duplicated(original$x),,drop=FALSE]
  last <- original[!duplicated(original$x,fromLast=TRUE),,drop=FALSE]
  d <- d[order(d$x,d$y),,drop=FALSE]
  envelope <- d[!duplicated(d$x,fromLast=TRUE),,drop=FALSE]
  minimum <- d[!duplicated(d$x),,drop=FALSE]
  reference_y <- round(envelope$y*1e9)
  lapply(temporal_candidates,function(candidate) {
    p <- candidate$points
    if(!length(p)) return(list(candidate=candidate$id,unavailable=TRUE))
    x <- vapply(p,function(v)v$hours,numeric(1)); y <- vapply(p,function(v)v$yield_bp,numeric(1))
    first_y <- y[!duplicated(x)]
    keep <- !duplicated(x,fromLast=TRUE); x<-x[keep];y<-y[keep]
    # Actual GeomLine one-sided limits, preserving its original tied row order.
    # Both curves are piecewise linear: extrema of their difference occur at
    # these breakpoint limits (native vertices are original read coordinates).
    pos <- pmax(1L,pmin(length(x),findInterval(envelope$x,x)))
    next_pos <- pmin(length(x),pos+1L)
    same <- x[pos]==envelope$x
    fraction <- ifelse(x[next_pos]==x[pos],0,(envelope$x-x[pos])/(x[next_pos]-x[pos]))
    interpolated <- y[pos]+pmax(0,pmin(1,fraction))*(first_y[next_pos]-y[pos])
    native_left <- ifelse(same,first_y[pos],interpolated)
    native_right <- ifelse(same,y[pos],interpolated)
    # Between timestamps interpolate to the NEXT FIRST ordinate, not its last
    # (post-tie) value. Collapsing ties before interpolation invents a diagonal
    # in place of a vertical jump and can spuriously exceed the display bound.
    predicted <- native_right
    error <- predicted-reference_y
    reference_pos <- pmax(1L,pmin(nrow(envelope),findInterval(x,envelope$x)))
    coordinate_error <- pmin(abs(x-envelope$x[reference_pos]),
      abs(x-envelope$x[pmin(nrow(envelope),reference_pos+1L)]))
    original_error <- max(abs(native_left-round(first$y*1e9)),abs(native_right-round(last$y*1e9)),
      abs(native_left-round(minimum$y*1e9)),abs(native_right-reference_y))
    sample <- unique(round(seq(1,nrow(envelope),length.out=min(256L,nrow(envelope)))))
    # All reference vertices are evaluated, not a sampled estimate of max error.
    list(candidate=candidate$id,reference_observations=nrow(d),
      reference_unique_times=nrow(envelope),reference_tied_observations=sum(duplicated(d$x)|duplicated(d$x,fromLast=TRUE)),
      native_observations=candidate$observations,native_rendered_points=length(p),
      maximum_envelope_error_bp=max(abs(error)),
      maximum_original_polyline_error_bp=original_error,
      maximum_error_hours=envelope$x[which.max(abs(error))],
      maximum_endpoint_coordinate_error_hours=max(abs(c(x[1]-min(d$x),tail(x,1)-max(d$x)))),
      unmatched_native_coordinates=sum(coordinate_error!=0),
      maximum_coordinate_error_hours=max(coordinate_error),
      reference_terminal_yield_bp=tail(reference_y,1),native_terminal_yield_bp=tail(y,1),
      samples=data.frame(hours=envelope$x[sample],reference_bp=reference_y[sample],native_bp=predicted[sample],residual_bp=error[sample]))
  })
}

audit_layer <- function(layer, data, limit=20000L, family="") {
  columns <- intersect(c("PANEL","group","x","xmin","xmax","y","ymin","ymax",
    "count","density","ncount","ndensity","colour","fill","alpha","size"), names(data))
  finite <- rep(TRUE,nrow(data))
  for (key in intersect(c("x","y"),names(data))) finite <- finite & is.finite(data[[key]])
  groups <- split(seq_len(nrow(data)), interaction(data$PANEL,data$group,drop=TRUE))
  totals <- lapply(groups,function(rows) list(panel=as.character(data$PANEL[rows[1]]),
    group=data$group[rows[1]],rows=length(rows),finite_xy=sum(finite[rows]),
    count=if("count" %in% names(data)) sum(data$count[rows],na.rm=TRUE) else NULL))
  bounded <- class(layer$stat)[1] %in% c("StatBin","StatBin2d","StatCount","StatSmooth","StatDensity") ||
    inherits(layer$geom,"GeomTile") || (family=="reads_per_hour.png" && inherits(layer$geom,"GeomLine"))
  result <- list(geom=class(layer$geom)[1], coordinates="post-scale/pre-coordinate",
    rows=nrow(data),finite_xy=sum(finite),nonfinite_xy=sum(!finite),groups=unname(totals),
    retention=if(bounded && nrow(data)<=limit) "complete_bounded_layer" else "bounded_summary",
    data=if(bounded && nrow(data)<=limit) data[,columns,drop=FALSE] else NULL)
  if (inherits(layer$geom,"GeomLine") && grepl("yield_by_length",family) && length(yield_candidates))
    result$yield_comparisons <- lapply(groups,function(rows) {
      d <- data[rows[finite[rows]],,drop=FALSE]
      if(!nrow(d)) return(NULL)
      list(panel=as.character(d$PANEL[1]),group=d$group[1],colour=d$colour[1],comparisons=yield_comparisons(d))
    })
  if (!bounded || nrow(data)>limit) {
    # Deterministic evenly spaced ROW samples are display diagnostics, not
    # complete data or a conservation proof. Count grids include EVERY finite row.
    rows <- if(nrow(data)) unique(round(seq(1,nrow(data),length.out=min(256L,nrow(data))))) else integer()
    result$sample <- data[rows,columns,drop=FALSE]
    if (all(c("x","y") %in% names(data)) && any(finite)) {
      x <- data$x[finite]; y <- data$y[finite]
      xr <- range(x); yr <- range(y)
      ix <- if(diff(xr)==0) rep(0L,length(x)) else pmin(63L,floor((x-xr[1])/diff(xr)*64))
      iy <- if(diff(yr)==0) rep(0L,length(y)) else pmin(39L,floor((y-yr[1])/diff(yr)*40))
      result$grid <- list(width=64L,height=40L,x_range=xr,y_range=yr,
        counts=as.integer(tabulate(1L+ix+64L*iy,nbins=2560L)),
        population=length(x),definition="all finite built-layer XY rows, upper endpoint clamped")
    }
  }
  if (inherits(layer$geom,"GeomLine") && grepl("yield_over_time",family)) {
    if(length(temporal_candidates)) result$temporal_comparisons <- lapply(groups,function(rows) {
      d <- data[rows[finite[rows]],,drop=FALSE]
      if(!nrow(d)) return(NULL)
      list(panel=as.character(d$PANEL[1]),group=d$group[1],colour=d$colour[1],
        comparisons=temporal_comparisons(d))
    })
    # Exact read-start cumulative values at bounded 15-minute queries. At tied
    # start times retain the maximum cumulative value (all tied reads counted).
    result$cumulative_queries <- lapply(groups,function(rows) {
      d <- data[rows[finite[rows]],,drop=FALSE]
      if(!nrow(d)) return(NULL)
      d <- d[order(d$x,d$y),,drop=FALSE]
      queries <- seq(0,max(d$x)+.25,by=.25)
      if(length(queries)>4096L) queries <- seq(0,max(d$x),length.out=4096L)
      indices <- findInterval(queries,d$x)
      list(panel=as.character(d$PANEL[1]),group=d$group[1],
        colour=if("colour" %in% names(d)) d$colour[1] else NULL,
        data=data.frame(x=queries,y=c(0,d$y)[indices+1L]))
    })
  }
  result
}
