cwlVersion: v1.0
class: CommandLineTool
baseCommand: echo
inputs:
  message:
    type: string
    inputBinding:
      position: 1
outputs:
  out:
    type: stdout
requirements:
  DockerRequirement:
    dockerPull: alpine:3.20
arguments: ["hello-from-tes-e2e"]
