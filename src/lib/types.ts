export type HeadwordMode = "exact" | "prefix" | "suffix" | "contains";
export type MatchMode = HeadwordMode | "fullText" | "idiom" | "example";
export type WritingMode = "auto" | "vertical" | "horizontal";

export interface DictionaryInfo {
  id: number;
  kind: "yomitan" | "epwing";
  title: string;
  revision: string;
  author: string;
  description: string;
  attribution: string;
  url: string;
  groupName: string;
  sortOrder: number;
  enabled: boolean;
  writingMode: WritingMode;
  termCount: number;
  metaCount: number;
  kanjiCount: number;
}

export interface SearchQuery {
  text: string;
  mode: MatchMode;
  dictIds: number[];
  limit: number;
  deinflect: boolean;
}

export interface SearchHit {
  termId: number;
  dictId: number;
  expression: string;
  reading: string;
  deinflection: string | null;
}

export interface TagInfo {
  name: string;
  category: string;
  notes: string;
}

export interface MetaInfo {
  dictTitle: string;
  data: any;
}

export interface Entry {
  termId: number;
  dictId: number;
  dictTitle: string;
  kind: "term" | "kanji";
  expression: string;
  reading: string;
  tags: TagInfo[];
  html: string;
  styles: string;
  frequencies: MetaInfo[];
  pitches: MetaInfo[];
  bookmarked: boolean;
}

export interface EntryView {
  entry: Entry;
  prev: SearchHit | null;
  next: SearchHit | null;
  writingMode: WritingMode;
}

export interface HistoryItem {
  id: number;
  termId: number;
  dictId: number;
  expression: string;
  reading: string;
  at: string;
  folder?: string;
  note?: string;
}

export interface ImportEvent {
  path: string;
  title: string;
  stage: string;
  done: number;
  total: number;
  error: string | null;
  finished: boolean;
}
