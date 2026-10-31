<?php

declare(strict_types=1);

namespace HyperUuid\Tests\Support;

/**
 * The argument coercion matrix: every public parameter of the API, called from a weak-mode file
 * and from a `declare(strict_types=1)` file with a fixed set of inputs, recording what each call
 * returned or threw and every warning or deprecation it raised.
 *
 * Strictness belongs to the file a call is written in, so the calls live in generated caller
 * files ({@see callerSource()}), one per mode, and this class only drives them. The same code
 * recorded 1.0's behaviour (tools/capture-v1-fixtures.php, into tests/fixtures/v1/coercion.json)
 * and checks the extension against it (tests/CoercionTest.php). It uses nothing but the
 * `HyperUuid\` API, so it runs against either.
 */
final class CoercionMatrix
{
    /** A version 7 UUID, the receiver of the instance methods. */
    public const V7 = '018bcfe5-6800-7000-8000-000000000001';

    /** The timestamp most cases pass beside the parameter under test. */
    public const MILLIS = 1700000000000;

    /**
     * Each case: the method and parameter under test, and the call, as PHP source, taking the
     * input as `$v` and reducing the result to something deterministic and JSON-encodable.
     *
     * @return array<string, string> `Class::method $param` => an expression over `$v`
     */
    public static function cases(): array
    {
        $v7 = "\\HyperUuid\\Uuid::parse('" . self::V7 . "')";
        $ms = (string) self::MILLIS;
        return [
            'Uuid::__construct $bytes' => 'bin2hex((new \HyperUuid\Uuid($v))->bytes())',
            'Uuid::parse $string' => '(string) \HyperUuid\Uuid::parse($v)',
            'Uuid::version $layout' => "{$v7}->version(\$v)",
            'Uuid::isRfc $version' => "{$v7}->isRfc(\$v)",
            'Uuid::isRfc $layout' => "{$v7}->isRfc(7, \$v)",
            'Uuid::equals $other' => "{$v7}->equals(\$v)",
            'Uuid::unixMillis $layout' => "{$v7}->unixMillis(\$v)",
            'Uuid::timestamp $throwOnMismatch' => '\HyperUuid\Uuid::nil()->timestamp($v)',
            'Uuid::timestamp $layout' => "{$v7}->timestamp(true, \$v)->format('Y-m-d\\TH:i:s.vP')",
            'HyperUuid::newV5 $namespace' => "(string) \\HyperUuid\\HyperUuid::newV5(\$v, 'name')",
            'HyperUuid::newV5 $name' =>
                '(string) \HyperUuid\HyperUuid::newV5(\HyperUuid\Namespaces::dns(), $v)',
            'HyperUuid::newV7 $unixMillis' => '\HyperUuid\HyperUuid::newV7($v)->unixMillis()',
            'HyperUuid::newV7Batch $count' => "count(\\HyperUuid\\HyperUuid::newV7Batch(\$v, {$ms}))",
            'HyperUuid::newV7Batch $unixMillis' => '\HyperUuid\HyperUuid::newV7Batch(1, $v)[0]->unixMillis()',
            'HyperUuid::newV7BatchBytes $count' =>
                "strlen(\\HyperUuid\\HyperUuid::newV7BatchBytes(\$v, {$ms}))",
            'HyperUuid::newV7BatchBytes $unixMillis' =>
                '(new \HyperUuid\Uuid(\HyperUuid\HyperUuid::newV7BatchBytes(1, $v)))->unixMillis()',
        ];
    }

    /** Whether a case's parameter is `DateTimeInterface|int|null $unixMillis`. */
    public static function takesTimestamps(string $case): bool
    {
        return str_ends_with($case, '$unixMillis');
    }

    /**
     * The inputs every parameter is called with.
     *
     * @return array<string, mixed>
     */
    public static function inputs(): array
    {
        return [
            'int' => 7,
            'int 1700000000000' => self::MILLIS,
            'negative int' => -1,
            'numeric string' => '7',
            'numeric string, leading whitespace' => ' 7',
            'numeric string, trailing whitespace' => '7 ',
            'integral float string' => '7.0',
            'fractional float string' => '7.5',
            'exponent string' => '1e3',
            'leading-numeric string' => '7abc',
            'non-numeric string' => 'abc',
            'empty string' => '',
            '16-byte string' => str_repeat("\x01", 16),
            'UUID string' => self::V7,
            'integral float' => 7.0,
            'fractional float' => 7.5,
            'float beyond int' => 1.0e20,
            'NAN' => NAN,
            'true' => true,
            'false' => false,
            'null' => null,
            'array' => [],
            'stdClass' => new \stdClass(),
            'Stringable "7"' => new class {
                public function __toString(): string
                {
                    return '7';
                }
            },
            'Uuid::nil()' => \HyperUuid\Uuid::nil(),
            'UuidLayout::SqlServer' => \HyperUuid\UuidLayout::SqlServer,
            'DateTimeImmutable' => new \DateTimeImmutable('2025-06-15T12:34:56.789999Z'),
        ];
    }

    /**
     * Extra inputs for the `$unixMillis` parameters: a `DateTimeInterface` whose
     * `getTimestamp()` returns something other than an int, which 1.0 put through its
     * arithmetic, `getTimestamp() * 1000 + intdiv((int) format('u'), 1000)`.
     *
     * @return array<string, mixed>
     */
    public static function timestampInputs(): array
    {
        $inputs = [];
        $returns = [
            'int' => 1700000000,
            'numeric string' => '1700000000',
            'numeric string, leading whitespace' => ' 1700000000',
            'integral float string' => '1700000000.0',
            'leading-numeric string' => '1700000000abc',
            'non-numeric string' => 'abc',
            'empty string' => '',
            'null' => null,
            'true' => true,
            'false' => false,
            'integral float' => 1700000000.0,
            'fractional float' => 1700000000.5,
            'array' => [],
            'stdClass' => new \stdClass(),
        ];
        foreach ($returns as $label => $value) {
            $inputs["getTimestamp() returning {$label}"] = self::dateTimeReturning($value);
        }
        return $inputs;
    }

    /** A `DateTimeImmutable` at 12:34:56.789 UTC whose `getTimestamp()` returns `$value`. */
    private static function dateTimeReturning(mixed $value): \DateTimeImmutable
    {
        $dateTime = new class ('2025-06-15T12:34:56.789000Z') extends \DateTimeImmutable {
            public mixed $timestamp = null;

            #[\ReturnTypeWillChange]
            public function getTimestamp()
            {
                return $this->timestamp;
            }
        };
        $dateTime->timestamp = $value;
        return $dateTime;
    }

    /**
     * A caller file: returns `[case => closure]`, each closure making its case's call from this
     * file, so with this file's strictness.
     */
    public static function callerSource(bool $strict): string
    {
        $source = "<?php\n\n" . ($strict ? "declare(strict_types=1);\n\n" : '') . "return [\n";
        foreach (self::cases() as $case => $call) {
            $source .= '    ' . var_export($case, true) . " => static fn(mixed \$v): mixed => {$call},\n";
        }
        return $source . "];\n";
    }

    /**
     * Runs every case of both modes, the caller files written into `$dir`, and returns the
     * matrix: `[mode][case][input] => outcome`.
     *
     * @return array<string, array<string, array<string, array<string, mixed>>>>
     */
    public static function run(string $dir): array
    {
        $matrix = [];
        foreach (['weak' => false, 'strict' => true] as $mode => $strict) {
            $file = $dir . "/{$mode}-caller.php";
            file_put_contents($file, self::callerSource($strict));
            // Messages name the file as PHP resolved it (macOS's /var is /private/var).
            $file = (string) realpath($file);
            $calls = require $file;
            foreach ($calls as $case => $call) {
                $inputs = self::inputs();
                if (self::takesTimestamps($case)) {
                    $inputs += self::timestampInputs();
                }
                foreach ($inputs as $label => $input) {
                    $matrix[$mode][$case][$label] = self::outcome($call, $input, $file);
                }
            }
        }
        return $matrix;
    }

    /**
     * What one call did: its result (or exception class and message) and every PHP error it
     * raised, in order. The caller file's path in a message is written `{caller}`.
     *
     * @return array<string, mixed>
     */
    private static function outcome(\Closure $call, mixed $input, string $file): array
    {
        $raised = [];
        set_error_handler(static function (int $level, string $message) use (&$raised): bool {
            $raised[] = self::levelName($level) . ': ' . $message;
            return true;
        });
        try {
            $outcome = ['result' => self::reduce($call($input))];
        } catch (\Throwable $e) {
            $outcome = [
                'exception' => get_class($e),
                'message' => str_replace($file, '{caller}', $e->getMessage()),
            ];
        } finally {
            restore_error_handler();
        }
        if ($raised !== []) {
            $outcome['errors'] = $raised;
        }
        return $outcome;
    }

    /** A result as recorded: a millisecond count within ten minutes of now is `now`. */
    private static function reduce(mixed $result): mixed
    {
        if (\is_int($result) && abs($result - (int) round(microtime(true) * 1000)) < 600_000) {
            return 'now';
        }
        return $result;
    }

    private static function levelName(int $level): string
    {
        return match ($level) {
            E_WARNING, E_USER_WARNING => 'Warning',
            E_NOTICE, E_USER_NOTICE => 'Notice',
            E_DEPRECATED, E_USER_DEPRECATED => 'Deprecated',
            default => "E_{$level}",
        };
    }
}
