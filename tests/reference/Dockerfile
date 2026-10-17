# Optional reference only. PoreGauge never requires R at runtime.
# The dated CRAN snapshot pins transitive R packages; the harness records the
# resolved image digest, sessionInfo and input/source hashes with every result.
FROM rocker/r-ver:4.4.1
RUN apt-get update && apt-get install -y --no-install-recommends libcurl4-openssl-dev libssl-dev libxml2-dev && rm -rf /var/lib/apt/lists/*
RUN Rscript -e 'install.packages(c("mgcv","ggplot2","viridis","plyr","reshape2","readr","yaml","scales","futile.logger","data.table","optparse","jsonlite"), repos="https://packagemanager.posit.co/cran/2024-07-01"); stopifnot(as.character(packageVersion("ggplot2"))=="3.5.1",as.character(packageVersion("mgcv"))=="1.9-1")'
ENV TZ=UTC LC_ALL=C.UTF-8
ENTRYPOINT ["Rscript"]
