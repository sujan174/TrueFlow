import { MessageSquare, ThumbsUp, ThumbsDown } from "lucide-react"

export function FeedbackWidget() {
  return (
    <div className="space-y-4">
      <p className="text-sm text-slate-600">
        How is TrueFlow working for you?
      </p>
      <div className="flex gap-3">
        <button className="flex-1 flex items-center justify-center gap-2 py-2.5 px-4 bg-slate-50 hover:bg-slate-100 rounded-lg transition-colors">
          <ThumbsUp className="w-4 h-4 text-green-600" />
          <span className="text-sm font-medium text-slate-700">Good</span>
        </button>
        <button className="flex-1 flex items-center justify-center gap-2 py-2.5 px-4 bg-slate-50 hover:bg-slate-100 rounded-lg transition-colors">
          <ThumbsDown className="w-4 h-4 text-amber-600" />
          <span className="text-sm font-medium text-slate-700">Needs work</span>
        </button>
      </div>
      <button className="w-full flex items-center justify-center gap-2 py-2.5 px-4 border border-slate-200 hover:bg-slate-50 rounded-lg transition-colors">
        <MessageSquare className="w-4 h-4 text-slate-500" />
        <span className="text-sm font-medium text-slate-600">Send feedback</span>
      </button>
    </div>
  )
}