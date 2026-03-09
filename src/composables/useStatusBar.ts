import { reactive } from 'vue'

export interface StatusTag {
  text: string
  type: 'success' | 'danger' | 'warning' | 'info'
}

export interface StatusSegment {
  label: string
  tag?: StatusTag
  extraTags?: StatusTag[]
}

const segments = reactive<StatusSegment[]>([])

export function useStatusBar() {
  const setSegments = (newSegments: StatusSegment[]) => {
    segments.splice(0, segments.length, ...newSegments)
  }
  const clearSegments = () => {
    segments.splice(0, segments.length)
  }
  return { segments, setSegments, clearSegments }
}
