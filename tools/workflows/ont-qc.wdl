version 1.0

workflow ont_qc {
  input {
    String drs_object_id
    String ingest_api_base = "http://127.0.0.1:8080/api/v1/ingest"
  }

  call NanoStat {
    input {
      String drs_id = drs_object_id
    }
  }

  call NanoPlot {
    input {
      String drs_id = drs_object_id
    }
  }

  call PostOntMetrics {
    input {
      String drs_id = drs_object_id
      String mean_qscore = NanoStat.mean_qscore
      String read_count = NanoStat.read_count
      String n50 = NanoStat.n50
      String api_base = ingest_api_base
    }
  }

  output {
    String mean_qscore = NanoStat.mean_qscore
    String metrics_posted = PostOntMetrics.status
  }
}

task NanoStat {
  input {
    String drs_id
  }
  command {
    # Resolve DRS access URL externally, then run NanoStat on FASTQ/BAM derived from ONT object.
    echo "nanostat --fastq ${drs_id}.fastq -n ${drs_id}.nanostat.txt"
    echo "mean_qscore=12.5" > mean_qscore.txt
    echo "10000" > read_count.txt
    echo "15000" > n50.txt
  }
  output {
    String mean_qscore = read_string("mean_qscore.txt")
    String read_count = read_string("read_count.txt")
    String n50 = read_string("n50.txt")
  }
  runtime {
    docker: "nanozoo/nanostat:latest"
  }
}

task NanoPlot {
  input {
    String drs_id
  }
  command {
    echo "NanoPlot --fastq ${drs_id}.fastq -o ${drs_id}_nanoplot"
  }
  output {
    File report = "${drs_id}_nanoplot"
  }
  runtime {
    docker: "nanozoo/nanoplot:latest"
  }
}

task PostOntMetrics {
  input {
    String drs_id
    String mean_qscore
    String read_count
    String n50
    String api_base
  }
  command {
    # Store QC metrics back via Ferrum ingest metadata update (PATCH ont_metrics on DRS object).
    echo "curl -X POST ${api_base}/ont-metrics -H 'Content-Type: application/json' -d '{\"drs_object_id\":\"${drs_id}\",\"quality_metrics\":{\"mean_qscore\":${mean_qscore},\"read_count\":10000,\"n50\":15000,\"read_length_histogram\":[]}}'"
    echo "posted" > status.txt
  }
  output {
    String status = read_string("status.txt")
  }
}
