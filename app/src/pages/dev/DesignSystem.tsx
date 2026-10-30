import {
  Bell as BellIcon,
  Check as CheckIcon,
  Mic as MicIcon,
  Plus as PlusIcon,
  Search as SearchIcon,
  Sparkles as SparklesIcon,
} from 'lucide-react';
import { useState } from 'react';

import Badge from '../../components/ui/Badge';
import Button from '../../components/ui/Button';
import Card from '../../components/ui/Card';
import Dialog from '../../components/ui/Dialog';
import EmptyState from '../../components/ui/EmptyState';
import Icon from '../../components/ui/Icon';
import IconButton from '../../components/ui/IconButton';
import { Input, Textarea } from '../../components/ui/Input';
import Kbd from '../../components/ui/Kbd';
import PageHeader from '../../components/ui/PageHeader';
import Select from '../../components/ui/Select';
import Skeleton from '../../components/ui/Skeleton';
import StatusDot from '../../components/ui/StatusDot';
import Switch from '../../components/ui/Switch';
import { ToastProvider, useToast } from '../../components/ui/Toast';
import Tooltip from '../../components/ui/Tooltip';

/**
 * Dev-only showcase of the UI kit (poor-man's Storybook). Every variant of
 * every primitive in one scroll — the reference page when migrating screens.
 * Route-gated by AppRoutes on `import.meta.env.DEV`.
 */

function Section(props: { title: string; children: React.ReactNode }) {
  return (
    <Card className="p-5">
      <h2 className="font-display text-lg font-medium text-ink-primary">{props.title}</h2>
      <div className="mt-4 flex flex-wrap items-center gap-3">{props.children}</div>
    </Card>
  );
}

function ToastProbe() {
  const { toast } = useToast();
  return (
    <Button variant="secondary" onClick={() => toast({ title: 'Saved', variant: 'success' })}>
      success toast
    </Button>
  );
}

const DesignSystem = () => {
  const [switchOn, setSwitchOn] = useState(true);
  const [selectValue, setSelectValue] = useState('');
  const [dialogOpen, setDialogOpen] = useState(false);

  return (
    <ToastProvider>
      <div className="mx-auto max-w-3xl space-y-6 p-6 pb-28">
        <PageHeader
          title="Design system"
          description="Aura UI kit — tokens and components. Dev-only page."
        />

        <Section title="Typography">
          <div className="w-full space-y-2">
            <p className="font-display text-3xl font-medium text-ink-primary">
              Cabinet Grotesk — display
            </p>
            <p className="text-2xl font-medium text-ink-primary">Title / 24 medium</p>
            <p className="text-base text-ink-primary">Body — Inter, the workhorse text.</p>
            <p className="text-sm text-ink-secondary">Secondary — captions and metadata.</p>
            <p className="text-xs text-ink-tertiary">Tertiary — timestamps, hints.</p>
            <p className="font-mono text-xs text-ink-secondary">JetBrains Mono — code / kbd</p>
          </div>
        </Section>

        <Section title="Buttons">
          <Button>Primary</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="danger">Danger</Button>
          <Button loading>Loading</Button>
          <Button leadingIcon={<Icon icon={PlusIcon} size="sm" />}>With icon</Button>
          <Button size="sm">Small</Button>
          <Button size="xs">Tiny</Button>
          <IconButton aria-label="Notifications" icon={<Icon icon={BellIcon} />} />
          <Tooltip content="Search (Ctrl+K)">
            <IconButton aria-label="Search" icon={<Icon icon={SearchIcon} />} />
          </Tooltip>
        </Section>

        <Section title="Badges & status">
          <Badge>neutral</Badge>
          <Badge variant="brand">brand</Badge>
          <Badge variant="success">connected</Badge>
          <Badge variant="warning">attention</Badge>
          <Badge variant="danger">error</Badge>
          <span className="inline-flex items-center gap-1.5 text-sm text-ink-secondary">
            <StatusDot tone="online" /> online
          </span>
          <span className="inline-flex items-center gap-1.5 text-sm text-ink-secondary">
            <StatusDot tone="busy" /> busy
          </span>
          <span className="inline-flex items-center gap-1.5 text-sm text-ink-secondary">
            <StatusDot tone="offline" /> offline
          </span>
          <span className="inline-flex items-center gap-1">
            <Kbd>Ctrl</Kbd>+<Kbd>Space</Kbd>
          </span>
        </Section>

        <Section title="Form controls">
          <div className="w-64">
            <Input aria-label="demo input" placeholder="Input…" />
          </div>
          <div className="w-64">
            <Input aria-label="demo input error" placeholder="Invalid…" error="Required field" />
          </div>
          <div className="w-64">
            <Textarea aria-label="demo textarea" placeholder="Textarea…" rows={2} />
          </div>
          <div className="w-64">
            <Select
              aria-label="demo select"
              value={selectValue}
              onValueChange={setSelectValue}
              placeholder="Pick a device…"
              options={[
                { value: 'a', label: 'Microphone (Realtek)' },
                { value: 'b', label: 'USB Headset' },
              ]}
            />
          </div>
          <Switch aria-label="demo switch" checked={switchOn} onCheckedChange={setSwitchOn} />
        </Section>

        <Section title="Feedback">
          <Skeleton className="h-4 w-40" />
          <Skeleton className="h-9 w-64 rounded-lg" />
          <ToastProbe />
          <Button variant="secondary" onClick={() => setDialogOpen(true)}>
            open dialog
          </Button>
          <Dialog
            open={dialogOpen}
            onOpenChange={setDialogOpen}
            title="Example dialog"
            description="Focus trapped, Esc and outside-click dismiss."
            footer={
              <>
                <Button variant="ghost" onClick={() => setDialogOpen(false)}>
                  Cancel
                </Button>
                <Button onClick={() => setDialogOpen(false)}>Confirm</Button>
              </>
            }>
            <p className="text-sm text-ink-secondary">
              Dialog body on Radix — the one modal for the whole app.
            </p>
          </Dialog>
        </Section>

        <Section title="Empty state">
          <div className="w-full">
            <EmptyState
              visual={<Icon icon={SparklesIcon} size="lg" />}
              title="Nothing here yet"
              description="Connect a channel or start a conversation to see it appear."
              action={<Button leadingIcon={<Icon icon={PlusIcon} size="sm" />}>Get started</Button>}
            />
          </div>
        </Section>

        <Section title="Icons (lucide)">
          <Icon icon={MicIcon} size="xs" />
          <Icon icon={MicIcon} size="sm" />
          <Icon icon={MicIcon} size="md" />
          <Icon icon={MicIcon} size="lg" />
          <Icon icon={CheckIcon} size="md" className="text-sage-600" />
        </Section>
      </div>
    </ToastProvider>
  );
};

export default DesignSystem;
