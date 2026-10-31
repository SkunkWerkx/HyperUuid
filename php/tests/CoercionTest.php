<?php

declare(strict_types=1);

namespace HyperUuid\Tests;

use HyperUuid\Tests\Support\CoercionMatrix;
use HyperUuid\Tests\Support\V1Differences;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;

/**
 * Every public parameter takes its arguments as 1.0's userland methods did: weak-mode callers
 * get PHP's standard coercion, with its deprecations and warnings; strict callers get the
 * `TypeError`s, worded as for a userland function (`..., called in F on line L`); and a
 * `DateTimeInterface` whose `getTimestamp()` returns a non-int goes through 1.0's arithmetic.
 * The expected outcomes are 1.0's own, recorded by tools/capture-v1-fixtures.php --coercion,
 * through the documented differences (V1Differences).
 */
final class CoercionTest extends TestCase
{
    private static string $scratch;

    /** @var array<string, array<string, array<string, array<string, mixed>>>> */
    private static array $actual;

    public static function setUpBeforeClass(): void
    {
        self::$scratch = sys_get_temp_dir() . '/hyperuuid-coercion-test-' . getmypid();
        if (!is_dir(self::$scratch)) {
            mkdir(self::$scratch, 0700, true);
        }
        self::$actual = CoercionMatrix::run(self::$scratch);
    }

    public static function tearDownAfterClass(): void
    {
        array_map('unlink', glob(self::$scratch . '/*') ?: []);
        rmdir(self::$scratch);
    }

    /**
     * Every minor's recording: the engine's own messages (a deprecation's wording, PHP 8.5's
     * NAN warnings) differ between minors, so 1.0 was recorded on each.
     *
     * @return array<string, array<string, mixed>>
     */
    private static function captures(): array
    {
        $json = (string) file_get_contents(__DIR__ . '/fixtures/v1/coercion.json');
        return json_decode($json, true, flags: JSON_THROW_ON_ERROR)['captures'];
    }

    /**
     * 1.0's recording on the running PHP minor.
     *
     * @return array<string, array<string, array<string, array<string, mixed>>>>
     */
    private static function expected(): array
    {
        $minor = PHP_MAJOR_VERSION . '.' . PHP_MINOR_VERSION;
        $captures = self::captures();
        if (!isset($captures[$minor])) {
            throw new \RuntimeException("coercion.json holds no recording of 1.0 on PHP {$minor}");
        }
        return $captures[$minor]['matrix'];
    }

    /** @return iterable<string, array{string, string}> */
    public static function modesAndCases(): iterable
    {
        foreach (self::expected() as $mode => $cases) {
            foreach (array_keys($cases) as $case) {
                yield "{$mode}: {$case}" => [$mode, $case];
            }
        }
    }

    #[DataProvider('modesAndCases')]
    public function testEachInputIsTakenAs1Point0TookIt(string $mode, string $case): void
    {
        $expected = self::expected()[$mode][$case];
        $this->assertSame(array_keys($expected), array_keys(self::$actual[$mode][$case] ?? []));
        foreach ($expected as $input => $outcome) {
            $outcome = V1Differences::expected($outcome);
            if (self::debugEngineWarnsTwice($mode, $case, $input)) {
                $outcome['errors'] = [...$outcome['errors'], ...$outcome['errors']];
            }
            $this->assertSame($outcome, self::$actual[$mode][$case][$input], "{$mode}, {$case}, input {$input}");
        }
    }

    /**
     * The one place a debug (ZEND_DEBUG) PHP differs: before an internal function runs, it
     * checks the arguments against their declared types itself, and on 8.5 that check raises
     * the "unexpected NAN value was coerced to bool" warning for a NAN given to a `bool`
     * parameter, which the call then raises again. 1.0's userland method was never checked
     * that way.
     */
    private static function debugEngineWarnsTwice(string $mode, string $case, string $input): bool
    {
        return ZEND_DEBUG_BUILD && PHP_VERSION_ID >= 80500 && $mode === 'weak'
            && $case === 'Uuid::timestamp $throwOnMismatch' && $input === 'NAN';
    }

    public function testTheFixtureCoversEveryMinorCaseAndInputInBothModes(): void
    {
        $this->assertSame(['8.3', '8.4', '8.5'], array_keys(self::captures()));
        foreach (self::captures() as $capture) {
            $this->assertSame(['weak', 'strict'], array_keys($capture['matrix']));
            $this->assertCovers($capture['matrix']);
        }
    }

    /** @param array<string, array<string, array<string, array<string, mixed>>>> $matrix */
    private function assertCovers(array $matrix): void
    {
        foreach ($matrix as $cases) {
            $this->assertSame(array_keys(CoercionMatrix::cases()), array_keys($cases));
            foreach ($cases as $case => $inputs) {
                $labels = array_keys(CoercionMatrix::inputs());
                if (CoercionMatrix::takesTimestamps($case)) {
                    $labels = [...$labels, ...array_keys(CoercionMatrix::timestampInputs())];
                }
                $this->assertSame($labels, array_keys($inputs), $case);
            }
        }
    }

    public function testTheMatrixTellsTheModesApart(): void
    {
        // Guards the harness itself: if strictness weren't the caller file's, the two modes
        // would record the same outcomes.
        $weak = self::$actual['weak']['HyperUuid::newV7 $unixMillis']['numeric string'];
        $strict = self::$actual['strict']['HyperUuid::newV7 $unixMillis']['numeric string'];
        $this->assertSame(['result' => 7], $weak);
        $this->assertSame('TypeError', $strict['exception'] ?? null);
    }
}
