# Optional, bounded observation of ALREADY built ggplot layers. No recomputation
# or modification of plot inputs/statistical results. Coordinates are post-scale,
# pre-coordinate transformation (e.g. histogram length boundaries are log10 bp).
audit_scale <- function(scale) list(
  transformation=scale$trans$name, limits=scale$get_limits(),
  trained_range=scale$range$range, breaks=scale$get_breaks())

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
    inherits(layer$geom,"GeomTile")
  result <- list(geom=class(layer$geom)[1], coordinates="post-scale/pre-coordinate",
    rows=nrow(data),finite_xy=sum(finite),nonfinite_xy=sum(!finite),groups=unname(totals),
    retention=if(bounded && nrow(data)<=limit) "complete_bounded_layer" else "bounded_summary",
    data=if(bounded && nrow(data)<=limit) data[,columns,drop=FALSE] else NULL)
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
