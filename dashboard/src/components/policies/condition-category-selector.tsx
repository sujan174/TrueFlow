"use client"

import { cn } from "@/lib/utils"
import { FIELD_CATEGORIES, type FieldCategory } from "@/lib/field-registry"

interface CategorySelectorProps {
  selectedCategoryId: string | null
  onSelect: (categoryId: string) => void
}

export function CategorySelector({ selectedCategoryId, onSelect }: CategorySelectorProps) {
  return (
    <div className="grid grid-cols-3 gap-3">
      {FIELD_CATEGORIES.map((category) => {
        const isSelected = selectedCategoryId === category.id
        const Icon = category.icon

        return (
          <button
            key={category.id}
            type="button"
            onClick={() => onSelect(category.id)}
            className={cn(
              "flex flex-col items-center gap-2 p-4 rounded-xl border text-center transition-all",
              isSelected
                ? "border-primary bg-primary/5 ring-2 ring-primary/20"
                : "border-border bg-card hover:bg-muted/50"
            )}
          >
            <div className={cn(
              "p-2 rounded-lg",
              isSelected
                ? "bg-primary/10 text-primary"
                : "bg-muted text-muted-foreground"
            )}>
              <Icon className="h-5 w-5" />
            </div>
            <span className={cn(
              "text-sm font-medium",
              isSelected ? "text-foreground" : "text-muted-foreground"
            )}>
              {category.label}
            </span>
            <p className="text-xs text-muted-foreground line-clamp-1">{category.description}</p>
          </button>
        )
      })}
    </div>
  )
}

export default CategorySelector