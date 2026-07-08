import type { DecisionAction } from '@/api/types'

/** 动作 → Badge 配色，保持全站一致的语义色。 */
export const actionBadgeClass: Record<DecisionAction, string> = {
  overweight:
    'border-transparent bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300',
  standard:
    'border-transparent bg-sky-100 text-sky-700 dark:bg-sky-500/20 dark:text-sky-300',
  delay:
    'border-transparent bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300',
  underweight:
    'border-transparent bg-orange-100 text-orange-700 dark:bg-orange-500/20 dark:text-orange-300',
  skip: 'border-transparent bg-rose-100 text-rose-700 dark:bg-rose-500/20 dark:text-rose-300',
}

export function formatMultiplier(value: number): string {
  return `${value.toFixed(2).replace(/0$/, '')}x`
}

export function formatCurrency(value: number, currency: string): string {
  return new Intl.NumberFormat(undefined, {
    style: 'currency',
    currency,
    maximumFractionDigits: 0,
  }).format(value)
}

export function formatPrice(value: number, currency: string): string {
  return new Intl.NumberFormat(undefined, {
    style: 'currency',
    currency,
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value)
}

export function formatSignedPct(value: number): string {
  return `${value >= 0 ? '+' : ''}${value.toFixed(1)}%`
}
