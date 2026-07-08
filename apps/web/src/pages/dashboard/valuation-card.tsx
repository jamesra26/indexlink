import { Gauge, Info } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Bar, BarChart, CartesianGrid, LabelList, XAxis, YAxis } from 'recharts'

import { useMarketOverview } from '@/api/queries'
import { Badge } from '@/components/ui/badge'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from '@/components/ui/chart'
import { Skeleton } from '@/components/ui/skeleton'
import { appLanguage } from '@/i18n'
import { actionBadgeClass, formatCurrency, formatMultiplier } from '@/lib/decision'
import { cn } from '@/lib/utils'

export function ValuationCard() {
  const { t, i18n } = useTranslation()
  const { data, isPending } = useMarketOverview()
  const lang = appLanguage(i18n.language)
  const chartConfig = {
    percentile: {
      label: t('dashboard.valuation.composite'),
      color: 'var(--chart-1)',
    },
  } satisfies ChartConfig
  const percentileData =
    data?.metrics.map((metric) => ({
      key: metric.key,
      label: t(`dashboard.valuation.metrics.${metric.key}`),
      description: t(`dashboard.valuation.metricDescriptions.${metric.key}`),
      percentile: metric.percentile,
    })) ?? []

  return (
    <Card className="lg:col-span-2">
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Gauge className="size-4 text-muted-foreground" />
          {t('dashboard.valuation.title')}
        </CardTitle>
        <CardDescription className="flex items-center gap-1">
          <Info className="size-3" />
          {t('dashboard.valuation.hint')}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {isPending || !data ? (
          <div className="space-y-3">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-40 w-full" />
          </div>
        ) : (
          <div className="space-y-6">
            <div className="flex flex-wrap items-end justify-between gap-4">
              <div>
                <div className="font-mono text-sm text-muted-foreground">
                  {data.symbol} · {data.name[lang]}
                </div>
                <div className="mt-1 flex items-baseline gap-2">
                  <span className="text-4xl font-semibold tabular-nums">
                    {data.compositePercentile}
                    <span className="text-xl text-muted-foreground">%</span>
                  </span>
                  <span className="text-sm text-muted-foreground">
                    {t('dashboard.valuation.composite')}
                  </span>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3 text-right sm:grid-cols-4">
                <div className="text-right">
                  <div className="text-xs text-muted-foreground">
                    {t('dashboard.valuation.suggestedAction')}
                  </div>
                  <Badge
                    className={cn('mt-1 h-6 px-3 text-sm', actionBadgeClass[data.suggestedAction])}
                  >
                    {t(`action.${data.suggestedAction}`)}
                  </Badge>
                </div>
                <div className="text-right">
                  <div className="text-xs text-muted-foreground">
                    {t('dashboard.valuation.multiplier')}
                  </div>
                  <div className="mt-1 text-xl font-semibold tabular-nums">
                    {formatMultiplier(data.suggestedMultiplier)}
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-xs text-muted-foreground">
                    {t('dashboard.valuation.expectedAmount')}
                  </div>
                  <div className="mt-1 text-xl font-semibold tabular-nums">
                    {formatCurrency(
                      data.baseDcaAmount * data.suggestedMultiplier,
                      data.currency,
                    )}
                  </div>
                </div>
                <div className="text-right">
                  <div className="text-xs text-muted-foreground">
                    {t('dashboard.valuation.nextDcaTime')}
                  </div>
                  <div className="mt-1 text-xl font-semibold tabular-nums">
                    {new Date(data.nextDcaTime).toLocaleString(i18n.language, {
                      month: 'short',
                      day: 'numeric',
                      hour: '2-digit',
                      minute: '2-digit',
                    })}
                  </div>
                </div>
              </div>
            </div>

            <ChartContainer
              config={chartConfig}
              className="h-56 w-full rounded-xl bg-muted/20 px-2 pb-2 pt-5"
            >
              <BarChart
                data={percentileData}
                margin={{ top: 20, right: 8, left: 0, bottom: 0 }}
              >
                <CartesianGrid vertical={false} strokeDasharray="3 3" />
                <XAxis
                  dataKey="label"
                  tickLine={false}
                  axisLine={false}
                  tickMargin={8}
                  interval={0}
                />
                <YAxis
                  domain={[0, 100]}
                  ticks={[0, 25, 50, 75, 100]}
                  tickLine={false}
                  axisLine={false}
                  width={36}
                  tickFormatter={(value: number) => `${value}%`}
                />
                <ChartTooltip
                  cursor={false}
                  content={
                    <ChartTooltipContent
                      hideLabel
                      formatter={(value, _name, _item, _index, payload) => {
                        const label =
                          typeof payload === 'object' &&
                          payload !== null &&
                          'label' in payload &&
                          typeof payload.label === 'string'
                            ? payload.label
                            : t('dashboard.valuation.composite')
                        const formattedValue =
                          typeof value === 'number' ? `${value}%` : String(value ?? '')
                        const description =
                          typeof payload === 'object' &&
                          payload !== null &&
                          'description' in payload &&
                          typeof payload.description === 'string'
                            ? payload.description
                            : ''

                        return (
                          <div className="flex max-w-60 flex-1 flex-col gap-2">
                            <div className="flex items-center justify-between gap-4">
                              <span className="font-medium text-foreground">{label}</span>
                              <span className="font-mono font-medium text-foreground tabular-nums">
                                {formattedValue}
                              </span>
                            </div>
                            {description && (
                              <p className="text-muted-foreground leading-relaxed">
                                {description}
                              </p>
                            )}
                          </div>
                        )
                      }}
                    />
                  }
                />
                <Bar
                  dataKey="percentile"
                  fill="var(--foreground)"
                  radius={[6, 6, 0, 0]}
                  maxBarSize={48}
                >
                  <LabelList
                    dataKey="percentile"
                    position="top"
                    className="fill-foreground font-mono text-xs font-semibold"
                    formatter={(value) => (typeof value === 'number' ? `${value}%` : '')}
                  />
                </Bar>
              </BarChart>
            </ChartContainer>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
